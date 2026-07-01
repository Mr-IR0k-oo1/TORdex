// ─── Compiler Pipeline ───────────────────────────────────────────────────
//
//   TDXL source  →  Lexer  →  Parser  →  AST  →  KIR  →  Optimizer  →  KIR  →  CodeGen  →  VM Bytecode
//
// This file contains the AST→KIR lowering and KIR→Bytecode codegen passes.

use tordex_vm::instruction::{
    classify, correlate, halt, infer, load_entity, load_graph, load_imm,
    load_str, search, similar, snapshot, store_val, timeline, traverse,
    Instruction,
};
use tordex_vm::Program;

use crate::ast::{BinExpr, Order, Stmt, Value};
use crate::error::Error;
use crate::kir::{FilterCond, KirOp, KirProgram};

// ─── Public API ──────────────────────────────────────────────────────────

pub fn compile(source: &str) -> Result<Program, Error> {
    // Phase 1-2: Lex & Parse → AST
    let mut lexer = crate::lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = crate::parser::Parser::new(tokens);
    let ast = parser.parse()?;

    // Phase 3: Lower AST → KIR
    let kir = lower_ast(&ast.statements);

    // Phase 4: Optimize KIR (compiler-level: dedup, DCE)
    let kir = crate::optimizer::optimize(kir);

    // Phase 4b: Query Optimize KIR (PostgreSQL-style: cost-based, statistics-driven)
    let kir = crate::query_optimizer::optimize_query(kir);

    // Phase 5: Codegen KIR → VM Bytecode
    let program = codegen_kir(&kir);

    Ok(program)
}

// ─── Phase 3: AST → KIR Lowering ─────────────────────────────────────────

pub(crate) fn lower_ast(stmts: &[Stmt]) -> KirProgram {
    let mut prog = KirProgram::new();

    // The pipeline accumulator — the output of the last statement becomes
    // the input to the next. We track which value ID holds the current result.
    let mut pipeline: Option<crate::kir::KirValueId> = None;

    for stmt in stmts {
        pipeline = lower_stmt(stmt, &mut prog, pipeline);
    }

    // Every pipeline ends with an explicit Return, making the dataflow fully
    // visible to the optimizer (Collect → Observe → Resolve → ... → Return).
    if let Some(last) = pipeline {
        prog.push(KirOp::Return { value_id: last });
    }

    prog.output = pipeline;
    prog
}

/// Lower a single AST statement to KIR ops.
///
/// `input` is the value ID of the current pipeline accumulator (output of
/// the previous statement). Returns the new pipeline value (output of this
/// statement).
fn lower_stmt(
    stmt: &Stmt,
    prog: &mut KirProgram,
    input: Option<crate::kir::KirValueId>,
) -> Option<crate::kir::KirValueId> {
    match stmt {
        Stmt::From(val) => {
            let source_id = lower_string(val, prog);
            let output = prog.alloc_id();
            prog.push(KirOp::Collect { output, source_id });
            Some(output)
        }

        Stmt::Match(expr) | Stmt::Where(expr) => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            let (field, op, value) = match expr {
                BinExpr::Field { field, op, value } => (field, op, value),
                _ => return Some(input),
            };
            let filter = FilterCond {
                field: field.clone(),
                op: format!("{:?}", op).to_lowercase(),
                value: value.as_string(),
            };
            let output = prog.alloc_id();
            prog.push(KirOp::Filter {
                output,
                input,
                filter,
            });
            Some(output)
        }

        Stmt::Traverse { relation, depth } => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            let relation_id = lower_string(relation, prog);
            let output = prog.alloc_id();
            prog.push(KirOp::Traverse {
                output,
                input,
                relation_id,
                depth: depth.map(|d| d as i32),
            });
            Some(output)
        }

        Stmt::Summarize => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            let output = prog.alloc_id();
            prog.push(KirOp::Observe { output, input });
            Some(output)
        }

        Stmt::Infer(val) => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            let rule_id = lower_string(val, prog);
            let output = prog.alloc_id();
            prog.push(KirOp::Infer {
                output,
                input,
                rule_id,
            });
            Some(output)
        }

        Stmt::Correlate { target } => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            let target_id = lower_string(target, prog);
            let output = prog.alloc_id();
            prog.push(KirOp::Correlate {
                output,
                input,
                target_id,
            });
            Some(output)
        }

        Stmt::Similar { target } => {
            let target_id = lower_string(target, prog);
            let output = prog.alloc_id();
            prog.push(KirOp::Similar {
                output,
                target_id,
                threshold: 0.8,
            });
            Some(output)
        }

        Stmt::Classify { class: _ } => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            let output = prog.alloc_id();
            prog.push(KirOp::Observe { output, input });
            Some(output)
        }

        Stmt::Timeline { from, to } => {
            let mut query = serde_json::json!({});
            if let Some(f) = from {
                query["from"] = serde_json::json!(f.as_string());
            }
            if let Some(t) = to {
                query["to"] = serde_json::json!(t.as_string());
            }
            let json_str = query.to_string();
            let query_id = lower_raw_string(&json_str, prog);
            let output = prog.alloc_id();
            prog.push(KirOp::Timeline { output, query_id });
            Some(output)
        }

        Stmt::Sort { field, order } => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            let ascending = matches!(order, Order::Asc);
            prog.push(KirOp::SortHint {
                input,
                field: field.as_string(),
                ascending,
            });
            Some(input) // pass through
        }

        Stmt::Limit(n) => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            prog.push(KirOp::LimitHint {
                input,
                limit: *n,
            });
            Some(input) // pass through
        }

        Stmt::Store(val) => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            let name_id = lower_string(val, prog);
            prog.push(KirOp::Store {
                value_id: input,
                name_id,
            });
            Some(input) // pass through value
        }

        Stmt::Export(val) => {
            let input = input.unwrap_or_else(|| {
                let dummy = prog.alloc_id();
                let kind = prog.alloc_id();
                prog.push(KirOp::ConstStr {
                    output: kind,
                    value: "_".into(),
                });
                prog.push(KirOp::Collect {
                    output: dummy,
                    source_id: kind,
                });
                dummy
            });
            let format_id = lower_string(val, prog);
            prog.push(KirOp::Export {
                value_id: input,
                format_id,
            });
            Some(input)
        }

        Stmt::Load(val) => {
            let source_id = lower_string(val, prog);
            let output = prog.alloc_id();
            prog.push(KirOp::Collect { output, source_id });
            Some(output)
        }

        Stmt::Filter(expr) => {
            // Filter without a prior FROM — treat same as Match
            lower_stmt(&Stmt::Match(expr.clone()), prog, input)
        }
    }
}

/// Lower an AST Value to a ConstStr KIR op. Returns the value ID.
fn lower_string(val: &Value, prog: &mut KirProgram) -> crate::kir::KirValueId {
    lower_raw_string(&val.as_string(), prog)
}

fn lower_raw_string(s: &str, prog: &mut KirProgram) -> crate::kir::KirValueId {
    let output = prog.alloc_id();
    prog.push(KirOp::ConstStr {
        output,
        value: s.to_string(),
    });
    output
}

// ─── Phase 5: KIR → VM Bytecode Codegen ─────────────────────────────────

/// Register convention for codegen:
///   R0 = pipeline accumulator
///   R1–R3 = temp string/operand registers
///   R4  = numeric temp (threshold)
///   R5  = depth
///   R7  = spare
const ACC: u8 = 0;
const TMP1: u8 = 1;
const TMP2: u8 = 2;
const TMP3: u8 = 3;
const NUM: u8 = 4;
const DEPTH: u8 = 5;

/// A register allocator that maps KIR value IDs to VM registers.
///
/// Since the pipeline is largely linear (output of one op → input of next),
/// a simple allocator suffices: strings go to TMP1/TMP2/TMP3 round-robin,
/// and the pipeline accumulator uses R0.
struct RegAlloc {
    /// Map from KIR value ID to VM register.
    map: std::collections::HashMap<crate::kir::KirValueId, u8>,
    /// Next temp register to assign (1, 2, or 3).
    next_tmp: u8,
    /// Set of value IDs that have been materialized (load_str emitted).
    materialized: std::collections::HashSet<crate::kir::KirValueId>,
}

impl RegAlloc {
    fn new() -> Self {
        Self {
            map: std::collections::HashMap::new(),
            next_tmp: TMP1,
            materialized: std::collections::HashSet::new(),
        }
    }

    /// Allocate a register for a KIR value ID.
    fn alloc(&mut self, id: crate::kir::KirValueId) -> u8 {
        if let Some(&reg) = self.map.get(&id) {
            return reg;
        }
        let reg = self.next_tmp;
        self.map.insert(id, reg);
        // round-robin through TMP1, TMP2, TMP3
        self.next_tmp = if reg == TMP1 {
            TMP2
        } else if reg == TMP2 {
            TMP3
        } else {
            TMP1
        };
        reg
    }

    /// Mark a value ID as already materialized (load_str emitted for it).
    fn mark_materialized(&mut self, id: crate::kir::KirValueId) {
        self.materialized.insert(id);
    }

    fn is_materialized(&self, id: crate::kir::KirValueId) -> bool {
        self.materialized.contains(&id)
    }
}

/// Codegen: lower KIR → VM bytecode (Vec<Instruction> + constant pool).
pub fn codegen_kir(program: &KirProgram) -> Program {
    let mut codegen = CodegenEngine::new();
    codegen.emit_all(program);
    codegen.finish()
}

struct CodegenEngine {
    consts: Vec<String>,
    instrs: Vec<Instruction>,
    regs: RegAlloc,
    /// Current pipeline value ID (value in R0).
    pipeline_val: Option<crate::kir::KirValueId>,
}

impl CodegenEngine {
    fn new() -> Self {
        Self {
            consts: Vec::new(),
            instrs: Vec::new(),
            regs: RegAlloc::new(),
            pipeline_val: None,
        }
    }

    fn add_string(&mut self, s: &str) -> u16 {
        if let Some(idx) = self.consts.iter().position(|x| x == s) {
            return idx as u16;
        }
        let idx = self.consts.len();
        self.consts.push(s.to_string());
        idx as u16
    }

    fn emit(&mut self, instr: Instruction) {
        self.instrs.push(instr);
    }

    fn emit_load_str(&mut self, id: crate::kir::KirValueId, s: &str) -> u8 {
        if self.regs.is_materialized(id) {
            return self.regs.alloc(id);
        }
        let reg = self.regs.alloc(id);
        let idx = self.add_string(s);
        self.emit(load_str(reg, idx));
        self.regs.mark_materialized(id);
        reg
    }

    fn finish(mut self) -> Program {
        self.emit(halt());
        let mut program = Program::new("tdxl_program");
        for s in &self.consts {
            program.add_constant(serde_json::Value::String(s.clone()));
        }
        program.instructions = self.instrs;
        program
    }

    fn emit_all(&mut self, program: &KirProgram) {
        // Pre-materialize all ConstStr ops
        for op in &program.ops {
            if let KirOp::ConstStr { output, value } = op {
                self.emit_load_str(*output, value);
            }
        }

        // Emit knowledge ops
        for op in &program.ops {
            self.emit_op(op);
        }
    }

    fn emit_op(&mut self, op: &KirOp) {
        match op {
            KirOp::Collect { output, source_id } => {
                // Collect loads an entity from the named source (same as SourceEntity).
                let source_reg = self.regs.alloc(*source_id);
                self.emit(load_entity(ACC, source_reg));
                self.pipeline_val = Some(*output);
            }

            KirOp::SourceEntity { output, kind_id } => {
                let kind_reg = self.regs.alloc(*kind_id);
                self.emit(load_entity(ACC, kind_reg));
                self.pipeline_val = Some(*output);
            }

            KirOp::SourceSearch { output, query_id } => {
                let query_reg = self.regs.alloc(*query_id);
                self.emit(search(ACC, query_reg));
                self.pipeline_val = Some(*output);
            }

            KirOp::Observe { output, input } => {
                // Observe transforms raw → structured (same as Classify in bytecode).
                if !self.is_pipeline(*input) {
                    self.emit_mov_to_acc(*input);
                }
                self.emit(classify(ACC, ACC));
                self.pipeline_val = Some(*output);
            }

            KirOp::Classify { output, input } => {
                if !self.is_pipeline(*input) {
                    self.emit_mov_to_acc(*input);
                }
                self.emit(classify(ACC, ACC));
                self.pipeline_val = Some(*output);
            }

            KirOp::LoadGraph { output, input } => {
                if !self.is_pipeline(*input) {
                    self.emit_mov_to_acc(*input);
                }
                self.emit(load_graph(ACC, ACC));
                self.pipeline_val = Some(*output);
            }

            KirOp::Resolve { output, input, strategy_id } => {
                // Resolve uses the strategy as a query to disambiguate the input.
                if !self.is_pipeline(*input) {
                    self.emit_mov_to_acc(*input);
                }
                let strategy_reg = self.regs.alloc(*strategy_id);
                self.emit(load_graph(TMP1, ACC));
                self.emit(search(ACC, strategy_reg));
                self.pipeline_val = Some(*output);
            }

            KirOp::Filter { output, input, filter } => {
                if !self.is_pipeline(*input) {
                    self.emit_mov_to_acc(*input);
                }
                // Emit load_graph to bring in facts, then search with filter
                self.emit(load_graph(TMP1, ACC));

                let filter_json = serde_json::json!({
                    "field": &filter.field,
                    "op": &filter.op,
                    "value": &filter.value,
                });
                let json_str = filter_json.to_string();
                let filter_id = *output; // use output ID for the filter string
                self.emit_load_str(filter_id, &json_str);
                let filter_reg = self.regs.alloc(filter_id);
                self.emit(search(ACC, filter_reg));
                self.pipeline_val = Some(*output);
            }

            KirOp::Traverse {
                output,
                input,
                relation_id,
                depth,
            } => {
                if !self.is_pipeline(*input) {
                    self.emit_mov_to_acc(*input);
                }
                let rel_reg = self.regs.alloc(*relation_id);
                self.emit(traverse(ACC, rel_reg));
                if let Some(d) = depth {
                    self.emit(load_imm(DEPTH, *d));
                }
                self.pipeline_val = Some(*output);
            }

            KirOp::Infer {
                output,
                input,
                rule_id,
            } => {
                if !self.is_pipeline(*input) {
                    self.emit_mov_to_acc(*input);
                }
                let rule_reg = self.regs.alloc(*rule_id);
                self.emit(infer(ACC, ACC, rule_reg));
                self.pipeline_val = Some(*output);
            }

            KirOp::Correlate {
                output,
                input,
                target_id,
            } => {
                if !self.is_pipeline(*input) {
                    self.emit_mov_to_acc(*input);
                }
                // Load target entity
                let tgt_reg = self.regs.alloc(*target_id);
                let target_idx = self.add_string("_target");
                self.emit(load_str(TMP2, target_idx));
                self.emit(load_entity(TMP3, tgt_reg));
                self.emit(correlate(ACC, TMP3));
                self.pipeline_val = Some(*output);
            }

            KirOp::Similar {
                output,
                target_id,
                threshold,
            } => {
                let tgt_reg = self.regs.alloc(*target_id);
                self.emit(load_imm(NUM, (*threshold * 10.0) as i32));
                self.emit(similar(ACC, tgt_reg, NUM));
                self.pipeline_val = Some(*output);
            }

            KirOp::Timeline { output, query_id } => {
                let query_reg = self.regs.alloc(*query_id);
                self.emit(timeline(ACC, query_reg));
                self.pipeline_val = Some(*output);
            }

            KirOp::Store { value_id, name_id } => {
                if !self.is_pipeline(*value_id) {
                    self.emit_mov_to_acc(*value_id);
                }
                let _name_reg = self.regs.alloc(*name_id);
                self.emit(store_val(ACC));
            }

            KirOp::Export { value_id, format_id } => {
                if !self.is_pipeline(*value_id) {
                    self.emit_mov_to_acc(*value_id);
                }
                let fmt_reg = self.regs.alloc(*format_id);
                // snapshot with kind derived from format string
                self.emit(snapshot(ACC, fmt_reg));
                self.pipeline_val = Some(*value_id);
            }

            KirOp::Snapshot {
                output,
                value_id,
                kind,
            } => {
                if !self.is_pipeline(*value_id) {
                    self.emit_mov_to_acc(*value_id);
                }
                self.emit(snapshot(ACC, *kind));
                self.pipeline_val = Some(*output);
            }

            KirOp::Return { value_id } => {
                // Mark the pipeline value; no VM instruction emitted.
                self.pipeline_val = Some(*value_id);
            }

            KirOp::SortHint { .. }
            | KirOp::LimitHint { .. }
            | KirOp::ConstStr { .. }
            | KirOp::ConstInt { .. }
            | KirOp::ConstFloat { .. } => {
                // Handled elsewhere; no-op during codegen
            }
        }
    }

    fn is_pipeline(&self, id: crate::kir::KirValueId) -> bool {
        self.pipeline_val == Some(id)
    }

    fn emit_mov_to_acc(&mut self, id: crate::kir::KirValueId) {
        let reg = self.regs.alloc(id);
        if reg != ACC {
            // Use a Mov instruction if available; for now just re-load.
            // The VM's mov instruction is: mov(dst, src)
            self.emit(tordex_vm::instruction::mov(ACC, reg));
        }
    }
}
