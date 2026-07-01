use std::collections::HashMap;

use crate::kir::{KirOp, KirProgram, KirValueId};

// ─── Optimization Passes ─────────────────────────────────────────────────

/// Run all optimization passes on a KIR program.
pub fn optimize(program: KirProgram) -> KirProgram {
    let program = eliminate_redundant_strings(program);
    let program = propagate_limits(program);
    let program = eliminate_dead_code(program);
    program
}

// ─── Pass 1: Redundant String Elimination ────────────────────────────────

/// Merge duplicate ConstStr ops that produce the same string.
/// All consumers are rewritten to point to the first occurrence.
fn eliminate_redundant_strings(program: KirProgram) -> KirProgram {
    let mut new_prog = KirProgram::new();
    let mut seen_strings: HashMap<String, KirValueId> = HashMap::new();
    let mut replacements: HashMap<KirValueId, KirValueId> = HashMap::new();

    for op in &program.ops {
        if let KirOp::ConstStr { output, value } = op {
            if let Some(&existing) = seen_strings.get(value) {
                replacements.insert(*output, existing);
            } else {
                seen_strings.insert(value.clone(), *output);
            }
        }
    }

    for op in &program.ops {
        if let KirOp::ConstStr { output, .. } = &op {
            if replacements.contains_key(output) {
                continue;
            }
        }
        new_prog.push(rewrite_inputs(&op, &replacements));
    }

    new_prog.output = program
        .output
        .map(|o| replacements.get(&o).copied().unwrap_or(o));
    new_prog
}

fn rewrite_inputs(op: &KirOp, repl: &HashMap<KirValueId, KirValueId>) -> KirOp {
    let r = |id: &KirValueId| repl.get(id).copied().unwrap_or(*id);

    match op.clone() {
        KirOp::Collect { output, source_id } => KirOp::Collect {
            output,
            source_id: r(&source_id),
        },
        KirOp::SourceEntity { output, kind_id } => KirOp::SourceEntity {
            output,
            kind_id: r(&kind_id),
        },
        KirOp::SourceSearch { output, query_id } => KirOp::SourceSearch {
            output,
            query_id: r(&query_id),
        },
        KirOp::Observe { output, input } => KirOp::Observe {
            output,
            input: r(&input),
        },
        KirOp::Classify { output, input } => KirOp::Classify {
            output,
            input: r(&input),
        },
        KirOp::LoadGraph { output, input } => KirOp::LoadGraph {
            output,
            input: r(&input),
        },
        KirOp::Filter { output, input, filter } => KirOp::Filter {
            output,
            input: r(&input),
            filter,
        },
        KirOp::Resolve {
            output,
            input,
            strategy_id,
        } => KirOp::Resolve {
            output,
            input: r(&input),
            strategy_id: r(&strategy_id),
        },
        KirOp::Traverse {
            output,
            input,
            relation_id,
            depth,
        } => KirOp::Traverse {
            output,
            input: r(&input),
            relation_id: r(&relation_id),
            depth,
        },
        KirOp::Correlate {
            output,
            input,
            target_id,
        } => KirOp::Correlate {
            output,
            input: r(&input),
            target_id: r(&target_id),
        },
        KirOp::Similar {
            output,
            target_id,
            threshold,
        } => KirOp::Similar {
            output,
            target_id: r(&target_id),
            threshold,
        },
        KirOp::Infer {
            output,
            input,
            rule_id,
        } => KirOp::Infer {
            output,
            input: r(&input),
            rule_id: r(&rule_id),
        },
        KirOp::Timeline { output, query_id } => KirOp::Timeline {
            output,
            query_id: r(&query_id),
        },
        KirOp::Return { value_id } => KirOp::Return {
            value_id: r(&value_id),
        },
        KirOp::Store { value_id, name_id } => KirOp::Store {
            value_id: r(&value_id),
            name_id: r(&name_id),
        },
        KirOp::Export { value_id, format_id } => KirOp::Export {
            value_id: r(&value_id),
            format_id: r(&format_id),
        },
        KirOp::Snapshot {
            output,
            value_id,
            kind,
        } => KirOp::Snapshot {
            output,
            value_id: r(&value_id),
            kind,
        },
        KirOp::SortHint { input, field, ascending } => KirOp::SortHint {
            input: r(&input),
            field,
            ascending,
        },
        KirOp::LimitHint { input, limit } => KirOp::LimitHint {
            input: r(&input),
            limit,
        },
        other => other,
    }
}

// ─── Pass 2: Limit Propagation ───────────────────────────────────────────

/// Propagate limit hints into adjacent operations (future: loop bounds).
fn propagate_limits(program: KirProgram) -> KirProgram {
    // Currently a no-op; reserved for future loop unrolling / bounds checking.
    program
}

// ─── Pass 3: Dead Code Elimination ────────────────────────────────────────

/// Remove ops whose output is never used as input and is not the program output.
fn eliminate_dead_code(program: KirProgram) -> KirProgram {
    let used = program.used_values();
    let mut new_prog = KirProgram::new();

    for op in program.ops {
        let keep = match op.output_id() {
            None => true,
            Some(id) => used.contains(&id),
        };
        if keep {
            new_prog.push(op);
        }
    }

    new_prog.output = program.output;
    new_prog
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kir::KirOp;

    fn make_test_prog() -> KirProgram {
        let mut prog = KirProgram::new();
        let c0 = prog.alloc_id();
        let _c1 = prog.alloc_id();
        let _c2 = prog.alloc_id();
        prog.push(KirOp::ConstStr {
            output: c0,
            value: "OnionServices".into(),
        });
        let _c1 = prog.alloc_id();
        prog.push(KirOp::ConstStr {
            output: _c1,
            value: "unused".into(),
        });
        let _c2 = prog.alloc_id();
        prog.push(KirOp::ConstStr {
            output: _c2,
            value: "LINKS_TO".into(),
        });
        let e0 = prog.alloc_id();
        prog.push(KirOp::Collect {
            output: e0,
            source_id: c0,
        });
        prog.push(KirOp::Store {
            value_id: _c2,
            name_id: c0,
        });
        prog.output = Some(_c2);
        prog
    }

    #[test]
    fn test_dead_code_elimination() {
        let prog = make_test_prog();
        let optimized = eliminate_dead_code(prog);
        // "unused" constant should be removed
        assert!(!optimized.ops.iter().any(|op| matches!(op, KirOp::ConstStr { value, .. } if value == "unused")));
        // Remaining: c0 (used), _c2 (output), Collect, Store
        let const_count = optimized
            .ops
            .iter()
            .filter(|op| matches!(op, KirOp::ConstStr { .. }))
            .count();
        assert_eq!(const_count, 2);
    }

    #[test]
    fn test_redundant_string_elimination() {
        let mut prog = KirProgram::new();
        let c0 = prog.alloc_id();
        let c1 = prog.alloc_id();
        prog.push(KirOp::ConstStr {
            output: c0,
            value: "OnionServices".into(),
        });
        prog.push(KirOp::ConstStr {
            output: c1,
            value: "OnionServices".into(),
        });
        let out = prog.alloc_id();
        prog.push(KirOp::Collect {
            output: out,
            source_id: c1,
        });
        prog.output = Some(out);

        let optimized = eliminate_redundant_strings(prog);
        let strs: Vec<_> = optimized
            .ops
            .iter()
            .filter_map(|op| {
                if let KirOp::ConstStr { value, .. } = op {
                    Some(value.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(strs, vec!["OnionServices"]);
    }

    #[test]
    fn test_full_pipeline() {
        let prog = make_test_prog();
        let optimized = optimize(prog);
        // Should have removed the "unused" const
        assert!(!optimized.ops.iter().any(|op| matches!(op, KirOp::ConstStr { value, .. } if value == "unused")));
    }
}
