/// Knowledge IR — an SSA-based intermediate representation between AST and VM bytecode.
///
/// Architecture (LLVM-style):
///   AST → KIR lowering → Optimizer → KIR → CodeGen → VM Bytecode
///
/// Each operation produces exactly one `KirValue` (identified by `KirValueId`).
/// Values flow into downstream ops as inputs, forming a dataflow graph.

/// Unique identifier for a KIR value (like an SSA virtual register).
pub type KirValueId = usize;

/// KIR types represent the kind of knowledge each value holds.
#[derive(Debug, Clone, PartialEq)]
pub enum KirType {
    Entity,
    FactSet,
    Fact,
    Relationship,
    Correlation,
    Timeline,
    StringVal,
    IntVal,
    FloatVal,
    BoolVal,
    Nil,
}

/// A structured filter condition for MATCH/WHERE statements.
#[derive(Debug, Clone)]
pub struct FilterCond {
    pub field: String,
    pub op: String,
    pub value: String,
}

/// A KIR operation — one node in the knowledge dataflow graph.
///
/// Every intelligence operation compiles into KIR, making the full pipeline
/// visible to the optimizer (Collect → Observe → Resolve → Correlate → Reason → Return).
#[derive(Debug, Clone)]
pub enum KirOp {
    // ── Intelligence Lifecycle: Collect ───────────────────────────────────
    /// Gather intelligence from a named source (FROM / LOAD).
    Collect {
        output: KirValueId,
        source_id: KirValueId,
    },

    /// Load an entity reference by kind string.
    SourceEntity {
        output: KirValueId,
        kind_id: KirValueId,
    },

    /// Load facts from semantic memory for a given entity.
    SourceSearch {
        output: KirValueId,
        query_id: KirValueId,
    },

    // ── Intelligence Lifecycle: Observe ───────────────────────────────────
    /// Process raw data into structured observations (CLASSIFY / SUMMARIZE).
    Observe {
        output: KirValueId,
        input: KirValueId,
    },

    /// Classify the input value.
    Classify {
        output: KirValueId,
        input: KirValueId,
    },

    /// Load the full fact graph for an entity.
    LoadGraph {
        output: KirValueId,
        input: KirValueId,
    },

    /// Filter a fact set by a structured condition (MATCH/WHERE).
    Filter {
        output: KirValueId,
        input: KirValueId,
        filter: FilterCond,
    },

    // ── Intelligence Lifecycle: Resolve ───────────────────────────────────
    /// Resolve an entity reference — disambiguate and enrich (WHERE / MATCH).
    Resolve {
        output: KirValueId,
        input: KirValueId,
        strategy_id: KirValueId,
    },

    /// Traverse a relationship from the current value.
    Traverse {
        output: KirValueId,
        input: KirValueId,
        relation_id: KirValueId,
        depth: Option<i32>,
    },

    // ── Intelligence Lifecycle: Correlate ─────────────────────────────────
    /// Correlate with another entity.
    Correlate {
        output: KirValueId,
        input: KirValueId,
        target_id: KirValueId,
    },

    /// Similarity search.
    Similar {
        output: KirValueId,
        target_id: KirValueId,
        threshold: f64,
    },

    // ── Intelligence Lifecycle: Reason ────────────────────────────────────
    /// Infer new facts using a rule (INFER).
    Infer {
        output: KirValueId,
        input: KirValueId,
        rule_id: KirValueId,
    },

    /// Build a timeline from episodic memory (TIMELINE).
    Timeline {
        output: KirValueId,
        query_id: KirValueId,
    },

    // ── Intelligence Lifecycle: Return ────────────────────────────────────
    /// Explicit pipeline output — marks the final value returned to the caller.
    Return {
        value_id: KirValueId,
    },

    // ── I/O Sinks ─────────────────────────────────────────────────────────
    /// Store a value to long-term memory.
    Store {
        value_id: KirValueId,
        name_id: KirValueId,
    },

    /// Export a value in a given format.
    Export {
        value_id: KirValueId,
        format_id: KirValueId,
    },

    /// Take a snapshot of the current state.
    Snapshot {
        output: KirValueId,
        value_id: KirValueId,
        kind: u8,
    },

    // ── Constants ────────────────────────────────────────────────────────
    /// A string constant (will be placed in the program's constant pool).
    ConstStr {
        output: KirValueId,
        value: String,
    },

    /// An integer constant.
    ConstInt {
        output: KirValueId,
        value: i64,
    },

    /// A float constant.
    ConstFloat {
        output: KirValueId,
        value: f64,
    },

    /// Sort hint (metadata, no-op in bytecode).
    SortHint {
        input: KirValueId,
        field: String,
        ascending: bool,
    },

    /// Limit hint (metadata, propagated as load_imm).
    LimitHint {
        input: KirValueId,
        limit: usize,
    },
}

impl KirOp {
    /// The output value ID produced by this operation (if any).
    pub fn output_id(&self) -> Option<KirValueId> {
        match self {
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
            | KirOp::ConstFloat { output, .. } => Some(*output),
            KirOp::Return { .. }
            | KirOp::Store { .. }
            | KirOp::Export { .. }
            | KirOp::SortHint { .. }
            | KirOp::LimitHint { .. } => None,
        }
    }

    /// All input value IDs consumed by this operation.
    pub fn input_ids(&self) -> Vec<KirValueId> {
        match self {
            KirOp::Collect { source_id, .. } => vec![*source_id],
            KirOp::SourceEntity { kind_id, .. } => vec![*kind_id],
            KirOp::SourceSearch { query_id, .. } => vec![*query_id],
            KirOp::Observe { input, .. } => vec![*input],
            KirOp::Classify { input, .. } => vec![*input],
            KirOp::LoadGraph { input, .. } => vec![*input],
            KirOp::Filter { input, .. } => vec![*input],
            KirOp::Resolve { input, strategy_id, .. } => vec![*input, *strategy_id],
            KirOp::Traverse { input, relation_id, .. } => vec![*input, *relation_id],
            KirOp::Infer { input, rule_id, .. } => vec![*input, *rule_id],
            KirOp::Correlate { input, target_id, .. } => vec![*input, *target_id],
            KirOp::Similar { target_id, .. } => vec![*target_id],
            KirOp::Timeline { query_id, .. } => vec![*query_id],
            KirOp::Return { value_id } => vec![*value_id],
            KirOp::Store { value_id, name_id } => vec![*value_id, *name_id],
            KirOp::Export { value_id, format_id } => vec![*value_id, *format_id],
            KirOp::Snapshot { value_id, .. } => vec![*value_id],
            KirOp::SortHint { input, .. } => vec![*input],
            KirOp::LimitHint { input, .. } => vec![*input],
            KirOp::ConstStr { .. }
            | KirOp::ConstInt { .. }
            | KirOp::ConstFloat { .. } => vec![],
        }
    }
}

/// A KIR program — a sequence of operations forming a dataflow graph.
#[derive(Debug, Clone)]
pub struct KirProgram {
    /// All operations in program order.
    pub ops: Vec<KirOp>,
    /// The value ID that is the final pipeline output (if any).
    pub output: Option<KirValueId>,
    /// Next available value ID.
    pub next_id: KirValueId,
}

impl KirProgram {
    pub fn new() -> Self {
        Self {
            ops: Vec::new(),
            output: None,
            next_id: 0,
        }
    }

    /// Allocate a new unique value ID.
    pub fn alloc_id(&mut self) -> KirValueId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Add an op and return the output value ID (if the op produces one).
    pub fn push(&mut self, op: KirOp) -> Option<KirValueId> {
        let out = op.output_id();
        self.ops.push(op);
        out
    }

    /// Collect all value IDs that are used as inputs to some op.
    pub fn used_values(&self) -> std::collections::HashSet<KirValueId> {
        let mut used = std::collections::HashSet::new();
        for op in &self.ops {
            for input in op.input_ids() {
                used.insert(input);
            }
        }
        if let Some(out) = self.output {
            used.insert(out);
        }
        used
    }
}
