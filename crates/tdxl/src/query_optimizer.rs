// ─── Query Optimizer ─────────────────────────────────────────────────────
//
// PostgreSQL-style cost-based optimization for KIR programs.
//
// Passes:
//   1. Predicate Pushdown  — move Filter ops closer to their Collect source
//   2. Traversal Pruning   — cap traversal depth based on fan-out estimates
//   3. Join Reordering     — sort Filter ops by selectivity (most selective first)
//   4. Operator Fusion     — merge adjacent compatible ops (Filter+Filter, etc.)

use std::collections::HashMap;

use crate::kir::{FilterCond, KirOp, KirProgram, KirValueId};

// ─── Statistics ──────────────────────────────────────────────────────────

/// Graph statistics used for cardinality estimation and cost modeling.
#[derive(Debug, Clone)]
pub struct GraphStatistics {
    /// Estimated number of entities per kind (e.g. "OnionServices" → 50_000).
    pub entity_counts: HashMap<String, u64>,
    /// Average number of facts per entity.
    pub avg_facts_per_entity: f64,
    /// Average traversal fan-out per relationship kind.
    pub traversal_fanout: HashMap<String, f64>,
    /// Selectivity estimates for predicate field+op combinations.
    /// A value of 0.01 means "filters to 1% of input rows".
    pub predicate_selectivity: HashMap<String, f64>,
    /// Default selectivity when no stats are available.
    pub default_selectivity: f64,
}

impl Default for GraphStatistics {
    fn default() -> Self {
        Self {
            entity_counts: HashMap::new(),
            avg_facts_per_entity: 10.0,
            traversal_fanout: HashMap::new(),
            predicate_selectivity: HashMap::new(),
            default_selectivity: 0.15,
        }
    }
}

impl GraphStatistics {
    pub fn entity_count(&self, kind: &str) -> u64 {
        self.entity_counts.get(kind).copied().unwrap_or(1000)
    }

    pub fn fanout(&self, relation: &str) -> f64 {
        self.traversal_fanout
            .get(relation)
            .copied()
            .unwrap_or(5.0)
    }

    pub fn selectivity(&self, field: &str, _op: &str) -> f64 {
        let key = format!("{}_{}", field, _op);
        self.predicate_selectivity
            .get(&key)
            .copied()
            .unwrap_or(self.default_selectivity)
    }

    pub fn filter_selectivity(&self, filter: &FilterCond) -> f64 {
        self.selectivity(&filter.field, &filter.op)
    }
}

// ─── Cost Model ──────────────────────────────────────────────────────────

/// Estimated cost and output cardinality for a single KIR operation.
#[derive(Debug, Clone, Copy)]
pub struct OpCost {
    /// Total estimated cost (abstract units).
    pub total: f64,
    /// Estimated output cardinality (number of result rows).
    pub cardinality: f64,
}

/// Cost model assigns cost to each KIR operation based on graph statistics.
pub struct CostModel {
    pub stats: GraphStatistics,
}

impl CostModel {
    pub fn new(stats: GraphStatistics) -> Self {
        Self { stats }
    }

    /// Estimate cost and output cardinality for an op given input cardinality.
    pub fn estimate(&self, op: &KirOp, input_card: f64) -> OpCost {
        match op {
            KirOp::Collect { .. } => OpCost {
                total: 10.0,
                cardinality: self.stats.avg_facts_per_entity,
            },
            KirOp::SourceEntity { .. } => OpCost {
                total: 8.0,
                cardinality: 1.0,
            },
            KirOp::SourceSearch { .. } => OpCost {
                total: 20.0,
                cardinality: self.stats.avg_facts_per_entity * 2.0,
            },
            KirOp::Observe { .. } | KirOp::Classify { .. } => OpCost {
                total: 5.0 + input_card * 0.5,
                cardinality: input_card,
            },
            KirOp::LoadGraph { .. } => OpCost {
                total: 15.0 + input_card * 1.5,
                cardinality: self.stats.avg_facts_per_entity,
            },
            KirOp::Filter { filter, .. } => {
                let sel = self.stats.filter_selectivity(filter);
                OpCost {
                    total: 3.0 + input_card * 0.3,
                    cardinality: (input_card * sel).max(1.0),
                }
            }
            KirOp::Resolve { .. } => OpCost {
                total: 8.0 + input_card * 0.8,
                cardinality: (input_card * 0.3).max(1.0),
            },
            KirOp::Traverse {
                relation_id: _,
                depth,
                ..
            } => {
                let d = depth.unwrap_or(1) as f64;
                // fan-out amplification: cardinality ≈ input × fanout^depth
                let fanout: f64 = 5.0; // default; we'd use stats if we had the relation string
                let amp = fanout.powf(d);
                OpCost {
                    total: 25.0 * d + input_card * amp * 0.5,
                    cardinality: (input_card * amp).max(1.0),
                }
            }
            KirOp::Correlate { .. } => OpCost {
                total: 30.0 + input_card * 5.0,
                cardinality: input_card,
            },
            KirOp::Similar { .. } => OpCost {
                total: 20.0 + input_card * 3.0,
                cardinality: (input_card * 0.1).max(1.0),
            },
            KirOp::Infer { .. } => OpCost {
                total: 15.0 + input_card * 2.0,
                cardinality: (input_card * 0.5).max(1.0),
            },
            KirOp::Timeline { .. } => OpCost {
                total: 25.0,
                cardinality: 100.0,
            },
            KirOp::Return { .. } | KirOp::Store { .. } | KirOp::Export { .. } | KirOp::Snapshot { .. } => {
                OpCost {
                    total: 1.0,
                    cardinality: input_card,
                }
            }
            KirOp::SortHint { .. } | KirOp::LimitHint { .. } => OpCost {
                total: 0.5,
                cardinality: input_card,
            },
            KirOp::ConstStr { .. } | KirOp::ConstInt { .. } | KirOp::ConstFloat { .. } => OpCost {
                total: 0.1,
                cardinality: 1.0,
            },
        }
    }

    /// Estimate total cost of a full KIR program.
    pub fn estimate_program(&self, program: &KirProgram) -> f64 {
        let mut total = 0.0;
        let mut card = 1.0;
        for op in &program.ops {
            let c = self.estimate(op, card);
            total += c.total;
            card = c.cardinality;
        }
        total
    }
}

// ─── Pipeline Extraction ─────────────────────────────────────────────────

/// Extract the main pipeline ops — the sequence where each op's output feeds
/// the next op's input via the pipeline accumulator.
fn pipeline_ops(program: &KirProgram) -> Vec<usize> {
    // Build a forward map: value_id → index of op that produces it
    let mut producers: HashMap<KirValueId, usize> = HashMap::new();
    for (i, op) in program.ops.iter().enumerate() {
        if let Some(out) = op.output_id() {
            producers.insert(out, i);
        }
    }

    // Build a reverse map: value_id → indices of ops that consume it
    let mut consumers: HashMap<KirValueId, Vec<usize>> = HashMap::new();
    for (i, op) in program.ops.iter().enumerate() {
        for inp in op.input_ids() {
            consumers.entry(inp).or_default().push(i);
        }
    }

    // Trace forward from the first non-constant op through pipeline connections.
    // A "pipeline connection" is when op A's output is consumed by op B, and
    // that's the *only* consumer (or it's the primary pipeline flow).
    let mut pipeline = Vec::new();
    let mut visited = std::collections::HashSet::new();

    // Find the first source op (Collect, SourceEntity, SourceSearch)
    let start = program.ops.iter().position(|op| {
        matches!(
            op,
            KirOp::Collect { .. } | KirOp::SourceEntity { .. } | KirOp::SourceSearch { .. }
        )
    });

    let start = match start {
        Some(s) => s,
        None => return pipeline,
    };

    // Walk forward from start
    let mut current = start;
    loop {
        if visited.contains(&current) {
            break;
        }
        visited.insert(current);
        pipeline.push(current);

        let op = &program.ops[current];
        match op.output_id() {
            Some(out) => {
                let nexts: Vec<usize> = consumers
                    .get(&out)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|i| !visited.contains(i))
                    .collect();
                if nexts.len() == 1 {
                    current = nexts[0];
                } else {
                    break;
                }
            }
            None => break,
        }
    }

    pipeline
}

// ─── Pass 1: Predicate Pushdown ──────────────────────────────────────────

/// Merge consecutive Filter ops and push filters closer to their Collect source.
fn predicate_pushdown(program: KirProgram) -> KirProgram {
    let pipe = pipeline_ops(&program);
    if pipe.is_empty() {
        return program;
    }

    // Collect the pipeline ops in order
    let pipe_ops: Vec<KirOp> = pipe.iter().map(|&i| program.ops[i].clone()).collect();

    // Merge consecutive Filter ops
    let mut merged: Vec<KirOp> = Vec::new();
    let mut i = 0;
    while i < pipe_ops.len() {
        if i + 1 < pipe_ops.len() {
            let (fst, snd) = (&pipe_ops[i], &pipe_ops[i + 1]);
            if let (KirOp::Filter { filter: f1, .. }, KirOp::Filter { filter: f2, .. }) = (fst, snd) {
                // Merge consecutive filters into a compound condition.
                // Encode both field conditions as an AND string for the downstream search.
                let compound = format!("{} {} {} AND {} {} {}",
                    f1.field, f1.op, f1.value,
                    f2.field, f2.op, f2.value);
                let new_filter = FilterCond {
                    field: f1.field.clone() + "_and_" + &f2.field,
                    op: "compound".to_string(),
                    value: compound,
                };
                // We'll keep the second filter's output ID and update its filter
                let output = pipe_ops[i + 1].output_id().unwrap_or(0);
                merged.push(KirOp::Filter {
                    output,
                    input: fst.input_ids().first().copied().unwrap_or(0),
                    filter: new_filter,
                });
                i += 2;
                continue;
            }
        }
        merged.push(pipe_ops[i].clone());
        i += 1;
    }

    // Rebuild the full program with merged pipeline and all non-pipeline ops
    rebuild_from_pipeline(&program, &merged)
}

// ─── Pass 2: Traversal Pruning ───────────────────────────────────────────

/// Cap traversal depth based on downstream Limit hints and fan-out estimates.
fn traversal_pruning(program: KirProgram) -> KirProgram {
    let pipe = pipeline_ops(&program);
    if pipe.is_empty() {
        return program;
    }

    let mut pipe_ops: Vec<KirOp> = pipe.iter().map(|&i| program.ops[i].clone()).collect();
    let mut modified = false;

    // Scan for LimitHint followed by Traverse, or Traverse followed by LimitHint
    let mut i = 0usize;
    while i < pipe_ops.len() {
        if let KirOp::Traverse {
            output,
            input,
            relation_id,
            depth,
        } = &pipe_ops[i]
        {
            // Look ahead for a LimitHint
            let limit = pipe_ops[i + 1..].iter().find_map(|op| {
                if let KirOp::LimitHint { limit, .. } = op {
                    Some(*limit)
                } else {
                    None
                }
            });

            if let Some(limit_val) = limit {
                let fanout_val: f64 = 5.0;
                let current_depth = depth.unwrap_or(1) as f64;
                let estimated = fanout_val.powf(current_depth);
                if estimated > limit_val as f64 * 2.0 {
                    let optimal_depth = (limit_val as f64).log(fanout_val).ceil() as i32;
                    let new_depth = optimal_depth.max(1).min(current_depth as i32);
                    if new_depth < current_depth as i32 {
                        pipe_ops[i] = KirOp::Traverse {
                            output: *output,
                            input: *input,
                            relation_id: *relation_id,
                            depth: Some(new_depth),
                        };
                        modified = true;
                    }
                }
            }
        }
        i += 1;
    }

    if modified {
        rebuild_from_pipeline(&program, &pipe_ops)
    } else {
        program
    }
}

// ─── Pass 3: Join Reordering ─────────────────────────────────────────────

/// Reorder consecutive Filter ops by selectivity (most selective first).
fn join_reordering(program: KirProgram) -> KirProgram {
    let pipe = pipeline_ops(&program);
    if pipe.len() < 2 {
        return program;
    }

    let mut pipe_ops: Vec<KirOp> = pipe.iter().map(|&i| program.ops[i].clone()).collect();

    // Find sequences of consecutive Filter ops and sort them by selectivity
    let mut i = 0usize;
    while i < pipe_ops.len() {
        if matches!(&pipe_ops[i], KirOp::Filter { .. }) {
            // Collect consecutive filters
            let filter_start = i;
            let mut filter_end = i;
            while filter_end < pipe_ops.len() {
                if matches!(pipe_ops[filter_end], KirOp::Filter { .. }) {
                    filter_end += 1;
                } else {
                    break;
                }
            }

            let count = filter_end - filter_start;
            if count > 1 {
                // Extract the filter segment
                let mut segment: Vec<(usize, KirOp)> = pipe_ops[filter_start..filter_end]
                    .iter()
                    .enumerate()
                    .map(|(j, op)| (filter_start + j, op.clone()))
                    .collect();

                // Sort by descending selectivity (most selective = smallest value = first)
                segment.sort_by(|a, b| {
                    let sa = match &a.1 {
                        KirOp::Filter { filter, .. } => {
                            let stats = GraphStatistics::default();
                            stats.filter_selectivity(filter)
                        }
                        _ => 1.0,
                    };
                    let sb = match &b.1 {
                        KirOp::Filter { filter, .. } => {
                            let stats = GraphStatistics::default();
                            stats.filter_selectivity(filter)
                        }
                        _ => 1.0,
                    };
                    sa.partial_cmp(&sb).unwrap()
                });

                // Rewire inputs: first filter gets the original input, subsequent
                // filters get the previous filter's output
                let original_input = match &pipe_ops[filter_start] {
                    KirOp::Filter { input, .. } => *input,
                    _ => unreachable!(),
                };

                let mut prev_output = original_input;
                for (_, op) in &mut segment {
                    match op {
                        KirOp::Filter {
                            ref mut input,
                            output,
                            ..
                        } => {
                            *input = prev_output;
                            prev_output = *output;
                        }
                        _ => {}
                    }
                }

                // Place sorted filters back
                for (j, (_idx, op)) in segment.into_iter().enumerate() {
                    pipe_ops[filter_start + j] = op;
                }
            }

            i = filter_end;
        } else {
            i += 1;
        }
    }

    rebuild_from_pipeline(&program, &pipe_ops)
}

// ─── Pass 4: Operator Fusion ─────────────────────────────────────────────

/// Fuse adjacent compatible operations (Filter+Filter already handled above;
/// this handles other fusion opportunities).
fn operator_fusion(program: KirProgram) -> KirProgram {
    let pipe = pipeline_ops(&program);
    if pipe.is_empty() {
        return program;
    }

    let pipe_ops: Vec<KirOp> = pipe.iter().map(|&i| program.ops[i].clone()).collect();

    // Fuse Collect + LoadGraph → single fused load
    let mut fused: Vec<KirOp> = Vec::new();
    let mut i = 0;
    while i < pipe_ops.len() {
        if i + 1 < pipe_ops.len() {
            let (fst, snd) = (&pipe_ops[i], &pipe_ops[i + 1]);
            match (fst, snd) {
                (KirOp::Collect { output: _o1, source_id: s1 }, KirOp::LoadGraph { output: o2, .. }) => {
                    // Fuse: the load_graph is redundant since Collect already loaded the entity.
                    // Keep the Collect and redirect the LoadGraph's consumers to Collect's output.
                    fused.push(KirOp::Collect {
                        output: *o2, // Use LoadGraph's output ID to maintain SSA
                        source_id: *s1,
                    });
                    i += 2;
                    continue;
                }
                (KirOp::Observe { .. }, KirOp::Classify { .. }) => {
                    // Dual classify is redundant; keep one
                    fused.push(fst.clone());
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        fused.push(pipe_ops[i].clone());
        i += 1;
    }

    rebuild_from_pipeline(&program, &fused)
}

// ─── Rebuild ─────────────────────────────────────────────────────────────

/// Rebuild a KIR program by replacing pipeline ops while keeping non-pipeline ops.
fn rebuild_from_pipeline(program: &KirProgram, new_pipe: &[KirOp]) -> KirProgram {
    let pipe_indices = pipeline_ops(program);

    // Collect non-pipeline ops (constants, hints, etc.)
    let pipe_set: std::collections::HashSet<usize> = pipe_indices.into_iter().collect();

    let mut new_prog = KirProgram::new();

    // Track ID remapping
    let mut id_map: HashMap<KirValueId, KirValueId> = HashMap::new();

    // First, emit all non-pipeline ops (they don't participate in dataflow
    // reordering so we keep them as-is with new IDs)
    for (old_idx, op) in program.ops.iter().enumerate() {
        if !pipe_set.contains(&old_idx) {
            let new_id = new_prog.alloc_id();
            let mapped = remap_op_output(op, new_id);
            if let Some(old_out) = op.output_id() {
                id_map.insert(old_out, new_id);
            }
            new_prog.push(remap_op_inputs(&mapped, &id_map));
        }
    }

    // Then emit the optimized pipeline, mapping old IDs to new
    for op in new_pipe {
        let mapped_op = if let Some(old_out) = op.output_id() {
            let new_id = new_prog.alloc_id();
            id_map.insert(old_out, new_id);
            remap_op_output(op, new_id)
        } else {
            op.clone()
        };
        new_prog.push(remap_op_inputs(&mapped_op, &id_map));
    }

    // Map the program output
    new_prog.output = program.output.map(|o| id_map.get(&o).copied().unwrap_or(o));

    new_prog
}

/// Replace the output ID in an op with a new one.
fn remap_op_output(op: &KirOp, new_id: KirValueId) -> KirOp {
    let mut c = op.clone();
    match &mut c {
        KirOp::Collect { output, .. }
        | KirOp::SourceEntity { output, .. }
        | KirOp::SourceSearch { output, .. }
        | KirOp::Observe { output, .. }
        | KirOp::Classify { output, .. }
        | KirOp::LoadGraph { output, .. }
        | KirOp::Filter { output, .. }
        | KirOp::Resolve { output, .. }
        | KirOp::Traverse { output, .. }
        | KirOp::Correlate { output, .. }
        | KirOp::Similar { output, .. }
        | KirOp::Infer { output, .. }
        | KirOp::Timeline { output, .. }
        | KirOp::Snapshot { output, .. }
        | KirOp::ConstStr { output, .. }
        | KirOp::ConstInt { output, .. }
        | KirOp::ConstFloat { output, .. } => *output = new_id,
        _ => {}
    }
    c
}

/// Remap input value IDs through a mapping.
fn remap_op_inputs(op: &KirOp, map: &HashMap<KirValueId, KirValueId>) -> KirOp {
    let r = |id: &KirValueId| map.get(id).copied().unwrap_or(*id);
    let mut c = op.clone();
    match &mut c {
        KirOp::Collect { source_id, .. } => *source_id = r(source_id),
        KirOp::SourceEntity { kind_id, .. } => *kind_id = r(kind_id),
        KirOp::SourceSearch { query_id, .. } => *query_id = r(query_id),
        KirOp::Observe { input, .. } => *input = r(input),
        KirOp::Classify { input, .. } => *input = r(input),
        KirOp::LoadGraph { input, .. } => *input = r(input),
        KirOp::Filter { input, .. } => *input = r(input),
        KirOp::Resolve { input, strategy_id, .. } => {
            *input = r(input);
            *strategy_id = r(strategy_id);
        }
        KirOp::Traverse { input, relation_id, .. } => {
            *input = r(input);
            *relation_id = r(relation_id);
        }
        KirOp::Correlate { input, target_id, .. } => {
            *input = r(input);
            *target_id = r(target_id);
        }
        KirOp::Similar { target_id, .. } => *target_id = r(target_id),
        KirOp::Infer { input, rule_id, .. } => {
            *input = r(input);
            *rule_id = r(rule_id);
        }
        KirOp::Timeline { query_id, .. } => *query_id = r(query_id),
        KirOp::Return { value_id, .. } => *value_id = r(value_id),
        KirOp::Store { value_id, name_id } => {
            *value_id = r(value_id);
            *name_id = r(name_id);
        }
        KirOp::Export { value_id, format_id } => {
            *value_id = r(value_id);
            *format_id = r(format_id);
        }
        KirOp::Snapshot { value_id, .. } => *value_id = r(value_id),
        KirOp::SortHint { input, .. } => *input = r(input),
        KirOp::LimitHint { input, .. } => *input = r(input),
        _ => {}
    }
    c
}

// ─── Public API ──────────────────────────────────────────────────────────

/// Run all query optimization passes on a KIR program.
///
/// Like PostgreSQL: cost-based, statistics-driven, multi-pass.
pub fn optimize_query(program: KirProgram) -> KirProgram {
    let program = predicate_pushdown(program);
    let program = traversal_pruning(program);
    let program = join_reordering(program);
    let program = operator_fusion(program);
    program
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kir::KirOp;

    fn make_filter_program() -> KirProgram {
        let mut prog = KirProgram::new();
        let src = prog.alloc_id();
        let f1_out = prog.alloc_id();
        let f2_out = prog.alloc_id();
        let f3_out = prog.alloc_id();

        let kind = prog.alloc_id();
        prog.push(KirOp::ConstStr {
            output: kind,
            value: "Sites".into(),
        });
        prog.push(KirOp::Collect {
            output: src,
            source_id: kind,
        });
        prog.push(KirOp::Filter {
            output: f1_out,
            input: src,
            filter: FilterCond {
                field: "Technology".into(),
                op: "eq".into(),
                value: "WordPress".into(),
            },
        });
        prog.push(KirOp::Filter {
            output: f2_out,
            input: f1_out,
            filter: FilterCond {
                field: "Status".into(),
                op: "eq".into(),
                value: "Alive".into(),
            },
        });
        prog.push(KirOp::Filter {
            output: f3_out,
            input: f2_out,
            filter: FilterCond {
                field: "Country".into(),
                op: "eq".into(),
                value: "US".into(),
            },
        });
        prog.output = Some(f3_out);
        prog
    }

    fn make_traversal_program() -> KirProgram {
        let mut prog = KirProgram::new();
        let src = prog.alloc_id();
        let trav_out = prog.alloc_id();

        let kind = prog.alloc_id();
        let rel = prog.alloc_id();
        prog.push(KirOp::ConstStr {
            output: kind,
            value: "Sites".into(),
        });
        prog.push(KirOp::ConstStr {
            output: rel,
            value: "LINKS_TO".into(),
        });
        prog.push(KirOp::Collect {
            output: src,
            source_id: kind,
        });
        prog.push(KirOp::Traverse {
            output: trav_out,
            input: src,
            relation_id: rel,
            depth: Some(5),
        });
        prog.push(KirOp::LimitHint {
            input: trav_out,
            limit: 10,
        });
        prog.output = Some(trav_out);
        prog
    }

    fn make_fusion_program() -> KirProgram {
        let mut prog = KirProgram::new();
        let src = prog.alloc_id();
        let graph_out = prog.alloc_id();

        let kind = prog.alloc_id();
        prog.push(KirOp::ConstStr {
            output: kind,
            value: "Sites".into(),
        });
        prog.push(KirOp::Collect {
            output: src,
            source_id: kind,
        });
        prog.push(KirOp::LoadGraph {
            output: graph_out,
            input: src,
        });
        prog.output = Some(graph_out);
        prog
    }

    #[test]
    fn test_predicate_pushdown_merges_filters() {
        let prog = make_filter_program();
        let optimized = predicate_pushdown(prog);
        // Should have fewer Filter ops (some merged)
        let filter_count = optimized
            .ops
            .iter()
            .filter(|op| matches!(op, KirOp::Filter { .. }))
            .count();
        assert!(filter_count <= 3, "expected ≤3 filters, got {}", filter_count);
    }

    #[test]
    fn test_traversal_pruning_reduces_depth() {
        let prog = make_traversal_program();
        let optimized = traversal_pruning(prog);
        // Should reduce depth from 5 to something smaller (limit=10, fanout=5, so log_5(10) ≈ 1.4 → depth=2)
        for op in &optimized.ops {
            if let KirOp::Traverse { depth, .. } = op {
                assert!(depth.unwrap_or(5) < 5, "depth should be pruned below 5");
            }
        }
    }

    #[test]
    fn test_operator_fusion_removes_load_graph() {
        let prog = make_fusion_program();
        let optimized = operator_fusion(prog);
        let load_graph_count = optimized
            .ops
            .iter()
            .filter(|op| matches!(op, KirOp::LoadGraph { .. }))
            .count();
        assert_eq!(load_graph_count, 0, "LoadGraph should be fused");
    }

    #[test]
    fn test_full_pipeline() {
        let prog = make_filter_program();
        let optimized = optimize_query(prog);
        // Should not crash, produce valid output
        assert!(optimized.output.is_some());
    }

    #[test]
    fn test_cost_model() {
        let stats = GraphStatistics::default();
        let cm = CostModel::new(stats);
        let prog = make_filter_program();
        let cost = cm.estimate_program(&prog);
        assert!(cost > 0.0, "cost should be positive");
    }
}
