use std::sync::Arc;

use time::OffsetDateTime;
use tordex_gpu::{GpuAccelerator, GpuEngine, SimilarityMetric, VectorIndex};
use tracing::{debug, info};

use crate::instruction::Opcode;
use crate::memory::MemoryManager;
use crate::program::Program;
use crate::register::{RegisterFile, RegisterValue};
use crate::stack::InferenceStack;
use crate::syscall::{SyscallArgs, SyscallHandler, SyscallNumber};

/// Execution context for the VM executor.
pub struct ExecutionContext {
    pub regs: RegisterFile,
    pub stack: InferenceStack,
    pc: usize,
    return_stack: Vec<usize>,
    program: Program,
    memory: Arc<MemoryManager>,
    syscall_handler: Arc<dyn SyscallHandler>,
    pub gpu: GpuEngine,
    pub running: bool,
    pub cycles: u64,
    pub max_cycles: u64,
}

impl ExecutionContext {
    pub fn new(
        program: Program,
        memory: Arc<MemoryManager>,
        syscall_handler: Arc<dyn SyscallHandler>,
    ) -> Self {
        Self {
            regs: RegisterFile::new(),
            stack: InferenceStack::default(),
            pc: program.entry_point,
            return_stack: Vec::new(),
            program,
            memory,
            syscall_handler,
            gpu: GpuEngine::new_cpu(),
            running: false,
            cycles: 0,
            max_cycles: 1_000_000,
        }
    }

    /// Set a custom GPU engine (e.g. for testing or future WgpuAccelerator).
    pub fn with_gpu(mut self, gpu: GpuEngine) -> Self {
        self.gpu = gpu;
        self
    }
}

/// Result of a single execution step.
#[derive(Debug, Clone)]
pub enum StepResult {
    Continue,
    Halted,
    Call(usize),
    Return(usize),
    Error(String),
}

/// The VM executor — runs the decode-execute cycle.
pub struct Executor;

impl Executor {
    /// Execute a program to completion. Returns when HALT is hit or an error
    /// occurs.
    pub fn execute(ctx: &mut ExecutionContext) -> Result<(), String> {
        ctx.running = true;
        ctx.cycles = 0;

        while ctx.running && ctx.cycles < ctx.max_cycles {
            let result = Self::step(ctx)?;
            ctx.cycles += 1;

            match result {
                StepResult::Halted => {
                    ctx.running = false;
                    return Ok(());
                }
                StepResult::Call(addr) => {
                    ctx.return_stack.push(ctx.pc);
                    ctx.pc = addr;
                }
                StepResult::Return(addr) => {
                    ctx.pc = addr;
                }
                StepResult::Error(msg) => {
                    ctx.running = false;
                    return Err(msg);
                }
                StepResult::Continue => {}
            }
        }

        if ctx.cycles >= ctx.max_cycles {
            return Err("execution exceeded max cycles".to_string());
        }

        Ok(())
    }

    /// Execute a single instruction and advance the program counter.
    pub fn step(ctx: &mut ExecutionContext) -> Result<StepResult, String> {
        let Some(instr) = ctx.program.instructions.get(ctx.pc) else {
            return Err(format!("invalid pc: {}", ctx.pc));
        };

        debug!(pc = ctx.pc, instr = %instr, "exec");

        let result = match instr.opcode {
            Opcode::Nop => StepResult::Continue,

            Opcode::LoadImm => {
                let val = RegisterValue::Int(instr.imm as i64);
                ctx.regs.write(instr.reg_a, val).map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::LoadStr => {
                let idx = instr.imm as u16;
                let s = ctx
                    .program
                    .get_constant(idx)
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default();
                ctx.regs
                    .write(instr.reg_a, RegisterValue::String(s))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Mov => {
                let val = ctx.regs.read(instr.reg_b)?.clone();
                ctx.regs.write(instr.reg_a, val).map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::LoadEntity => {
                let kind = ctx.regs.read(instr.reg_b)?.to_json().to_string();
                let entity = RegisterValue::Entity {
                    kind,
                    id: instr.imm.to_string(),
                };
                ctx.regs.write(instr.reg_a, entity).map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::LoadGraph => {
                let entity = ctx.regs.read(instr.reg_b)?.clone();
                let query = tordex_memory::SemanticQuery {
                    subjects: Some(vec![entity.to_json().to_string()]),
                    ..Default::default()
                };
                let results = ctx.memory.query_semantic(&query).map_err(|e| e.to_string())?;
                let facts: Vec<RegisterValue> = results
                    .into_iter()
                    .map(|f| RegisterValue::String(serde_json::to_string(&f).unwrap_or_default()))
                    .collect();
                ctx.regs
                    .write(instr.reg_a, RegisterValue::Facts(facts))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Traverse => {
                let entity = ctx.regs.read(instr.reg_b)?.to_json().to_string();
                let query = tordex_memory::SemanticQuery {
                    subjects: Some(vec![entity.clone()]),
                    limit: 100,
                    ..Default::default()
                };
                let results = ctx.memory.query_semantic(&query).map_err(|e| e.to_string())?;
                // GPU-accelerated: re-rank traversal results by embedding similarity
                let query_vec = ctx.gpu.embed(&entity);
                let candidate_vecs: Vec<Vec<f32>> = results
                    .iter()
                    .map(|f| ctx.gpu.embed(&format!("{} {} {}", f.subject, f.predicate, f.object)))
                    .collect();
                let index = VectorIndex::new(candidate_vecs, Vec::new());
                let ranked = ctx.gpu.vector_search(&query_vec, &index, results.len());
                let connected: Vec<RegisterValue> = ranked
                    .into_iter()
                    .map(|(idx, _)| {
                        RegisterValue::String(
                            serde_json::to_string(&results[idx]).unwrap_or_default(),
                        )
                    })
                    .collect();
                ctx.regs
                    .write(instr.reg_a, RegisterValue::Facts(connected))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Compare => {
                let a = ctx.regs.read(instr.reg_a)?.clone();
                let b = ctx.regs.read(instr.reg_b)?.clone();
                let equal =
                    serde_json::to_string(&a.to_json()).unwrap_or_default()
                    == serde_json::to_string(&b.to_json()).unwrap_or_default();
                ctx.regs
                    .write(instr.reg_a, RegisterValue::Boolean(equal))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Fact => {
                let subj = ctx.regs.read(instr.reg_a)?.clone();
                let pred = ctx.regs.read(instr.reg_b)?.clone();
                let obj = ctx.regs.read(instr.extra)?.clone();

                let fact = tordex_memory::SemanticFact {
                    id: ulid::Ulid::new(),
                    subject: subj.to_json().to_string(),
                    predicate: pred.to_json().to_string(),
                    object: obj.to_json(),
                    confidence: 1.0,
                    source_ids: vec!["vm".to_string()],
                    created_at: OffsetDateTime::now_utc(),
                    ttl: None,
                };

                ctx.memory.store_semantic_fact(fact).map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Query => {
                let query_str = ctx.regs.read(instr.reg_b)?.to_json().to_string();
                let query = SemanticQueryBuilder::from_json(&query_str).unwrap_or_default();
                let results = ctx.memory.query_semantic(&query).map_err(|e| e.to_string())?;

                let facts: Vec<RegisterValue> = results
                    .into_iter()
                    .map(|f| RegisterValue::String(serde_json::to_string(&f).unwrap_or_default()))
                    .collect();

                ctx.regs
                    .write(instr.reg_a, RegisterValue::Facts(facts))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Push => {
                let val = ctx.regs.read(instr.reg_a)?.clone();
                ctx.stack.push(val).map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Pop => {
                let val = ctx.stack.pop().map_err(|e| e.to_string())?;
                ctx.regs.write(instr.reg_a, val).map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Dup => {
                ctx.stack.dup().map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Swap => {
                ctx.stack.swap().map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Jmp => {
                let target = (ctx.pc as i32 + instr.imm) as usize;
                ctx.pc = target;
                return Ok(StepResult::Continue);
            }

            Opcode::Jz => {
                let val = ctx.regs.read(instr.reg_a)?;
                if !val.is_truthy() {
                    let target = (ctx.pc as i32 + instr.imm) as usize;
                    ctx.pc = target;
                    return Ok(StepResult::Continue);
                }
                StepResult::Continue
            }

            Opcode::Jnz => {
                let val = ctx.regs.read(instr.reg_a)?;
                if val.is_truthy() {
                    let target = (ctx.pc as i32 + instr.imm) as usize;
                    ctx.pc = target;
                    return Ok(StepResult::Continue);
                }
                StepResult::Continue
            }

            Opcode::Call => {
                let target = (ctx.pc as i32 + instr.imm) as usize;
                return Ok(StepResult::Call(target));
            }

            Opcode::Ret => {
                let addr = ctx.return_stack.pop().unwrap_or(ctx.pc + 1);
                return Ok(StepResult::Return(addr));
            }

            Opcode::Halt => {
                return Ok(StepResult::Halted);
            }

            Opcode::Relate => {
                let _src = ctx.regs.read(instr.reg_a)?;
                let _kind = ctx.regs.read(instr.reg_b)?;
                let _tgt = ctx.regs.read(instr.extra)?;
                // Relationship creation — future: bridge to graph
                info!(src = %_src.to_json(), kind = %_kind.to_json(), tgt = %_tgt.to_json(), "relate");
                StepResult::Continue
            }

            Opcode::Infer => {
                let subj = ctx.regs.read(instr.reg_b)?.to_json().to_string();
                let pred = ctx.regs.read(instr.extra)?.to_json().to_string();
                let query = tordex_memory::SemanticQuery {
                    subjects: Some(vec![subj]),
                    predicates: Some(vec![pred]),
                    ..Default::default()
                };
                let results = ctx.memory.query_semantic(&query).map_err(|e| e.to_string())?;
                let facts: Vec<RegisterValue> = results
                    .into_iter()
                    .map(|f| RegisterValue::String(serde_json::to_string(&f).unwrap_or_default()))
                    .collect();
                ctx.regs
                    .write(instr.reg_a, RegisterValue::Facts(facts))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Similar => {
                let value = ctx.regs.read(instr.reg_b)?.clone();
                let threshold_reg = ctx.regs.read(instr.extra)?;
                let threshold = threshold_reg.as_int().unwrap_or(8) as u32;
                let query = tordex_memory::SemanticQuery {
                    subjects: value.to_json().as_str().map(|s| vec![s.to_string()]),
                    limit: 20,
                    ..Default::default()
                };
                let results = ctx.memory.query_semantic(&query).map_err(|e| e.to_string())?;

                // GPU-accelerated: re-rank candidates by embedding similarity
                let input_text = value.to_json().to_string();
                let query_vec = ctx.gpu.embed(&input_text);
                let candidate_vecs: Vec<Vec<f32>> = results
                    .iter()
                    .map(|f| ctx.gpu.embed(&format!("{} {} {}", f.subject, f.predicate, f.object)))
                    .collect();
                let index = VectorIndex::new(candidate_vecs, Vec::new());
                let ranked = ctx.gpu.vector_search(&query_vec, &index, results.len());

                let similar: Vec<RegisterValue> = ranked
                    .into_iter()
                    .filter(|(_, score)| *score >= threshold as f32 / 10.0)
                    .map(|(idx, _)| {
                        RegisterValue::String(
                            serde_json::to_string(&results[idx]).unwrap_or_default(),
                        )
                    })
                    .collect();
                ctx.regs
                    .write(instr.reg_a, RegisterValue::Facts(similar))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Classify => {
                let content = ctx.regs.read(instr.reg_b)?.to_json();
                let content_str = content.as_str().unwrap_or("");
                // GPU-enhanced: use embedding + similarity against known class prototypes
                let vec = ctx.gpu.embed(content_str);
                let classes: [(&str, &[f32]); 5] = [
                    ("url", &ctx.gpu.embed("http://example.com/page")[..]),
                    ("document", &ctx.gpu.embed("long form text document with multiple paragraphs of content")[..]),
                    ("email", &ctx.gpu.embed("user@example.com subject line body")[..]),
                    ("numeric_id", &ctx.gpu.embed("1234567890")[..]),
                    ("text", &ctx.gpu.embed("general text content")[..]),
                ];
                let mut best = ("unknown", -1.0f32);
                for (label, proto) in &classes {
                    let sim = ctx.gpu.similarity(&vec, proto, SimilarityMetric::Cosine);
                    if sim > best.1 {
                        best = (*label, sim);
                    }
                }
                ctx.regs
                    .write(instr.reg_a, RegisterValue::String(best.0.to_string()))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Reason => {
                let fact_json = ctx.regs.read(instr.reg_b)?.to_json();
                let query = tordex_memory::SemanticQuery {
                    subjects: fact_json.as_str().map(|s| vec![s.to_string()]),
                    limit: 50,
                    ..Default::default()
                };
                let results = ctx.memory.query_semantic(&query).map_err(|e| e.to_string())?;
                // GPU-accelerated: score derived facts by embedding similarity
                let input_text = fact_json.to_string();
                let query_vec = ctx.gpu.embed(&input_text);
                let mut scored: Vec<(usize, f32)> = results
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let fact_text = format!("{} {} {}", f.subject, f.predicate, f.object);
                        let fact_vec = ctx.gpu.embed(&fact_text);
                        let sim = ctx.gpu.similarity(&query_vec, &fact_vec, SimilarityMetric::Cosine);
                        (i, sim)
                    })
                    .collect();
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                let derived: Vec<RegisterValue> = scored
                    .into_iter()
                    .filter(|(_, score)| *score >= 0.35)
                    .map(|(idx, _)| {
                        RegisterValue::String(
                            serde_json::to_string(&results[idx]).unwrap_or_default(),
                        )
                    })
                    .collect();
                ctx.regs
                    .write(instr.reg_a, RegisterValue::Facts(derived))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Correlate => {
                let a = ctx.regs.read(instr.reg_a)?.to_json();
                let b = ctx.regs.read(instr.reg_b)?.to_json();
                // GPU-accelerated: compute embedding similarity for actual correlation
                let a_text = a.as_str().unwrap_or("");
                let b_text = b.as_str().unwrap_or("");
                let vec_a = ctx.gpu.embed(a_text);
                let vec_b = ctx.gpu.embed(b_text);
                let score = ctx.gpu.similarity(&vec_a, &vec_b, SimilarityMetric::Cosine);
                let label = if score > 0.85 {
                    "strong"
                } else if score > 0.6 {
                    "moderate"
                } else if score > 0.3 {
                    "weak"
                } else {
                    "none"
                };
                let pair = serde_json::json!({
                    "entity_a": a,
                    "entity_b": b,
                    "correlation": label,
                    "score": score,
                });
                ctx.regs
                    .write(instr.reg_a, RegisterValue::String(pair.to_string()))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Timeline => {
                let query_json = ctx.regs.read(instr.reg_b)?.to_json();
                // GPU-accelerated: build timeline by embedding episodes and ranking by temporal relevance
                let query_text = query_json.as_str().unwrap_or("").to_string();
                let query_vec = ctx.gpu.embed(&query_text);
                let all_facts = ctx.memory.query_semantic(&tordex_memory::SemanticQuery {
                    limit: 200,
                    ..Default::default()
                }).map_err(|e| e.to_string())?;
                let mut scored: Vec<(usize, f32)> = all_facts
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let text = format!("{} {} {}", f.subject, f.predicate, f.object);
                        let vec = ctx.gpu.embed(&text);
                        let sim = ctx.gpu.similarity(&query_vec, &vec, SimilarityMetric::Cosine);
                        (i, sim)
                    })
                    .collect();
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                let episodes: Vec<serde_json::Value> = scored
                    .into_iter()
                    .take(50)
                    .filter(|(_, s)| *s > 0.3)
                    .map(|(idx, score)| {
                        let f = &all_facts[idx];
                        serde_json::json!({
                            "fact": serde_json::json!({"subject": f.subject, "predicate": f.predicate, "object": f.object}),
                            "relevance": score,
                            "timestamp": f.created_at.to_string(),
                        })
                    })
                    .collect();
                let timeline = serde_json::json!({
                    "query": query_json,
                    "status": "built",
                    "total": episodes.len(),
                    "episodes": episodes,
                });
                ctx.regs
                    .write(instr.reg_a, RegisterValue::String(timeline.to_string()))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Search => {
                let query_val = ctx.regs.read(instr.reg_a)?;
                let kind_val = ctx.regs.read(instr.reg_b)?;
                let results = ctx.memory.query_semantic(&tordex_memory::SemanticQuery {
                    subjects: query_val.to_json().as_str().map(|s| vec![s.to_string()]),
                    predicates: kind_val.to_json().as_str().map(|s| vec![s.to_string()]),
                    limit: 50,
                    ..Default::default()
                }).map_err(|e| e.to_string())?;
                let items: Vec<RegisterValue> = results
                    .into_iter()
                    .map(|f| RegisterValue::String(serde_json::to_string(&f).unwrap_or_default()))
                    .collect();
                ctx.regs
                    .write(instr.reg_a, RegisterValue::Facts(items))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Index => {
                let _doc = ctx.regs.read(instr.reg_a)?;
                let _idx = ctx.regs.read(instr.reg_b)?;
                info!(doc = %_doc.to_json(), "index");
                StepResult::Continue
            }

            Opcode::Rank => {
                let results_val = ctx.regs.read(instr.reg_b)?;
                // GPU-accelerated: rank fact results by embedding score
                let facts_slot = match &results_val {
                    RegisterValue::Facts(facts) => (*facts).clone(),
                    RegisterValue::String(s) => vec![RegisterValue::String((*s).clone())],
                    _ => Vec::new(),
                };
                let query_for_ranking = ctx.regs.read(instr.reg_a)
                    .ok().map(|v| v.to_json().to_string())
                    .unwrap_or_default();
                let query_vec = ctx.gpu.embed(&query_for_ranking);
                let scored: Vec<(String, f32)> = facts_slot
                    .iter()
                    .map(|rv| {
                        let text = rv.to_json().to_string();
                        let vec = ctx.gpu.embed(&text);
                        let sim = ctx.gpu.similarity(&query_vec, &vec, SimilarityMetric::Cosine);
                        (text, sim)
                    })
                    .collect();
                let ranked = ctx.gpu.rank(&scored, scored.len());
                let result_regs: Vec<RegisterValue> = ranked
                    .into_iter()
                    .map(|(s, _)| RegisterValue::String(s))
                    .collect();
                ctx.regs
                    .write(instr.reg_a, RegisterValue::Facts(result_regs))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Recall => {
                let id_str = ctx.regs.read(instr.reg_b)?.as_str().map(String::from);
                if let Some(id) = id_str.and_then(|s| ulid::Ulid::from_string(&s).ok()) {
                    let query = tordex_memory::LTQuery {
                        ids: Some(vec![id.to_string()]),
                        limit: 1,
                        ..Default::default()
                    };
                    let entries = ctx.memory.retrieve_ltm(&query).map_err(|e| e.to_string())?;
                    let val = entries
                        .first()
                        .map(|e| RegisterValue::String(serde_json::to_string(e).unwrap_or_default()))
                        .unwrap_or(RegisterValue::Nil);
                    ctx.regs.write(instr.reg_a, val).map_err(|e| e.to_string())?;
                }
                StepResult::Continue
            }

            Opcode::Consolidate => {
                // STM → LTM consolidation placeholder
                info!("consolidate");
                StepResult::Continue
            }

            Opcode::Forget => {
                let _id = ctx.regs.read(instr.reg_a)?;
                info!(id = %_id.to_json(), "forget");
                StepResult::Continue
            }

            Opcode::Store => {
                let val = ctx.regs.read(instr.reg_a)?.clone();
                let entry = tordex_memory::LTEntry {
                    id: ulid::Ulid::new(),
                    kind: "vm_stored".to_string(),
                    content: val.to_json(),
                    source_ids: vec!["vm::store".to_string()],
                    confidence: 1.0,
                    created_at: OffsetDateTime::now_utc(),
                    last_accessed_at: OffsetDateTime::now_utc(),
                    access_count: 0,
                    ttl: None,
                };
                let id = ctx.memory.store_ltm(entry).map_err(|e| e.to_string())?;
                ctx.regs
                    .write(instr.reg_a, RegisterValue::String(id.to_string()))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::Snapshot => {
                let kind = instr.reg_b;
                let now = OffsetDateTime::now_utc();
                let snapshot = serde_json::json!({
                    "kind": kind,
                    "timestamp": now.to_string(),
                    "type": match kind {
                        0 => "graph",
                        1 => "memory",
                        _ => "full",
                    },
                    "status": "captured",
                });
                ctx.regs
                    .write(instr.reg_a, RegisterValue::String(snapshot.to_string()))
                    .map_err(|e| e.to_string())?;
                StepResult::Continue
            }

            Opcode::SysCall => {
                let num = SyscallNumber::from_i32(instr.imm)
                    .ok_or_else(|| format!("invalid syscall number: {}", instr.imm))?;
                let args = SyscallArgs {
                    number: num,
                    arg0: instr.reg_a as i64,
                    arg1: instr.reg_b as i64,
                    arg2: instr.extra as i64,
                    data: None,
                };
                let result = ctx.syscall_handler.handle(&args);
                if !result.success {
                    return Err(result.error.unwrap_or_else(|| "syscall failed".to_string()));
                }
                StepResult::Continue
            }

            Opcode::Emit => {
                let _data = ctx.regs.read(instr.reg_a)?;
                let args = SyscallArgs {
                    number: SyscallNumber::EmitEvent,
                    arg0: 0,
                    arg1: 0,
                    arg2: 0,
                    data: Some(_data.to_json()),
                };
                ctx.syscall_handler.handle(&args);
                StepResult::Continue
            }
        };

        ctx.pc += 1;
        Ok(result)
    }
}

// ─── Helper ──────────────────────────────────────────────────────────────────

struct SemanticQueryBuilder;

impl SemanticQueryBuilder {
    fn from_json(json: &str) -> Option<tordex_memory::SemanticQuery> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        Some(tordex_memory::SemanticQuery {
            subjects: v.get("subjects").and_then(|s| serde_json::from_value(s.clone()).ok()),
            predicates: v.get("predicates").and_then(|s| serde_json::from_value(s.clone()).ok()),
            min_confidence: v.get("min_confidence").and_then(|c| c.as_f64()).unwrap_or(0.0),
            limit: v.get("limit").and_then(|l| l.as_u64()).unwrap_or(100) as usize,
        })
    }
}
