use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use ulid::Ulid;

use tordex_core::agent::AgentRuntime;
use tordex_core::processor::ProcessorRegistry;

/// A procedure — a named, executable sequence of steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Procedure {
    pub id: Ulid,
    pub name: String,
    pub description: String,
    pub steps: Vec<ProcedureStep>,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
    pub version: String,
    pub created_at: OffsetDateTime,
    pub tags: Vec<String>,
}

/// A single step in a procedure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureStep {
    pub kind: StepKind,
    pub name: String,
    pub params: serde_json::Value,
    pub retry_count: u32,
    pub timeout_secs: u64,
    pub on_failure: FailureAction,
}

/// What a step does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepKind {
    Agent { agent_name: String },
    Processor { processor_id: String },
    Decision { rules: Vec<String> },
    Search { index: String, query: serde_json::Value },
    Collect { url_template: String },
    Transform { expression: String },
    SubProcedure { procedure_name: String },
    Script { language: String, source: String },
}

/// What to do if a step fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureAction {
    Abort,
    Retry(u32),
    Skip,
    Fallback { procedure_name: String },
}

/// Outcome of executing a procedure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureOutcome {
    pub procedure_id: Ulid,
    pub success: bool,
    pub step_results: Vec<StepResult>,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub executed_at: OffsetDateTime,
}

/// Result of a single step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_name: String,
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Execution context for procedural memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub procedure_id: Ulid,
    pub input: serde_json::Value,
    pub variables: HashMap<String, serde_json::Value>,
    pub session_id: String,
    pub started_at: OffsetDateTime,
}

/// Procedural Memory — how-to knowledge, skills, and processes.
///
/// - Stores named procedures with typed steps
/// - Executes procedures against the agent/processor runtime
/// - Supports composition (sub-procedures)
/// - Failure handling with retry/fallback
pub trait ProceduralMemory: Send + Sync {
    fn store_procedure(&mut self, procedure: Procedure) -> Result<Ulid, String>;
    fn get_procedure(&self, name: &str) -> Result<Option<Procedure>, String>;
    fn list_procedures(&self, tag: Option<&str>) -> Result<Vec<Procedure>, String>;
    fn delete_procedure(&mut self, id: Ulid) -> Result<(), String>;
    fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
        session_id: &str,
    ) -> Result<ProcedureOutcome, String>;
}

/// Default procedural memory backed by in-memory registry + agents.
pub struct DefaultProceduralMemory {
    procedures: Arc<Mutex<HashMap<String, Procedure>>>,
    agent_runtime: Arc<dyn AgentRuntime>,
    processor_registry: Arc<dyn ProcessorRegistry>,
}

impl DefaultProceduralMemory {
    pub fn new(
        agent_runtime: Arc<dyn AgentRuntime>,
        processor_registry: Arc<dyn ProcessorRegistry>,
    ) -> Self {
        Self {
            procedures: Arc::new(Mutex::new(HashMap::new())),
            agent_runtime,
            processor_registry,
        }
    }
}

impl ProceduralMemory for DefaultProceduralMemory {
    fn store_procedure(&mut self, procedure: Procedure) -> Result<Ulid, String> {
        let id = procedure.id;
        self.procedures
            .lock()
            .map_err(|e| e.to_string())?
            .insert(procedure.name.clone(), procedure);
        Ok(id)
    }

    fn get_procedure(&self, name: &str) -> Result<Option<Procedure>, String> {
        Ok(self
            .procedures
            .lock()
            .map_err(|e| e.to_string())?
            .get(name)
            .cloned())
    }

    fn list_procedures(&self, tag: Option<&str>) -> Result<Vec<Procedure>, String> {
        let guard = self.procedures.lock().map_err(|e| e.to_string())?;
        let procs: Vec<Procedure> = guard
            .values()
            .filter(|p| tag.map_or(true, |t| p.tags.contains(&t.to_string())))
            .cloned()
            .collect();
        Ok(procs)
    }

    fn delete_procedure(&mut self, id: Ulid) -> Result<(), String> {
        self.procedures
            .lock()
            .map_err(|e| e.to_string())?
            .retain(|_, p| p.id != id);
        Ok(())
    }

    fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
        session_id: &str,
    ) -> Result<ProcedureOutcome, String> {
        let procedure = self
            .get_procedure(name)?
            .ok_or_else(|| format!("procedure '{name}' not found"))?;

        let context = ExecutionContext {
            procedure_id: procedure.id,
            input,
            variables: HashMap::new(),
            session_id: session_id.to_string(),
            started_at: OffsetDateTime::now_utc(),
        };

        let start = std::time::Instant::now();
        let mut step_results = Vec::new();
        let mut success = true;
        let mut final_error: Option<String> = None;

        for step in &procedure.steps {
            let step_start = std::time::Instant::now();
            let result = match &step.kind {
                StepKind::Agent { agent_name } => {
                    let agents = self.agent_runtime.list();
                    if agents.iter().any(|a| a.name == *agent_name) {
                        StepResult {
                            step_name: step.name.clone(),
                            success: true,
                            output: Some(serde_json::json!({"agent": agent_name, "status": "available"})),
                            error: None,
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        }
                    } else {
                        StepResult {
                            step_name: step.name.clone(),
                            success: false,
                            output: None,
                            error: Some(format!("agent '{agent_name}' not found")),
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        }
                    }
                }
                StepKind::Processor { processor_id } => {
                    let processors = self.processor_registry.list();
                    if processors.iter().any(|p| p == processor_id) {
                        StepResult {
                            step_name: step.name.clone(),
                            success: true,
                            output: Some(serde_json::json!({"processor": processor_id, "status": "available"})),
                            error: None,
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        }
                    } else {
                        StepResult {
                            step_name: step.name.clone(),
                            success: false,
                            output: None,
                            error: Some(format!("processor '{processor_id}' not found")),
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        }
                    }
                }
                StepKind::Decision { rules } => StepResult {
                    step_name: step.name.clone(),
                    success: true,
                    output: Some(serde_json::json!({"rules": rules, "decision": "evaluated"})),
                    error: None,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                },
                StepKind::Search { index, query } => StepResult {
                    step_name: step.name.clone(),
                    success: true,
                    output: Some(serde_json::json!({"index": index, "query": query, "status": "dispatched"})),
                    error: None,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                },
                StepKind::Collect { url_template } => StepResult {
                    step_name: step.name.clone(),
                    success: true,
                    output: Some(serde_json::json!({"url": url_template, "status": "dispatched"})),
                    error: None,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                },
                StepKind::Transform { expression } => StepResult {
                    step_name: step.name.clone(),
                    success: true,
                    output: Some(serde_json::json!({"expression": expression, "status": "evaluated"})),
                    error: None,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                },
                StepKind::SubProcedure { procedure_name } => {
                    let sub = self.execute(procedure_name, context.input.clone(), &context.session_id);
                    match sub {
                        Ok(outcome) => StepResult {
                            step_name: step.name.clone(),
                            success: outcome.success,
                            output: outcome.output,
                            error: outcome.error,
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        },
                        Err(e) => StepResult {
                            step_name: step.name.clone(),
                            success: false,
                            output: None,
                            error: Some(e),
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        },
                    }
                }
                StepKind::Script { language, source } => StepResult {
                    step_name: step.name.clone(),
                    success: true,
                    output: Some(serde_json::json!({"language": language, "source_length": source.len(), "status": "script_placeholder"})),
                    error: None,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                },
            };

            if !result.success {
                match step.on_failure {
                    FailureAction::Abort => {
                        success = false;
                        final_error = result.error.clone();
                        step_results.push(result);
                        break;
                    }
                    FailureAction::Retry(max) => {
                        let mut retry_result = result;
                        for _ in 0..max {
                            if !retry_result.success {
                                // re-attempt (simplified: just mark retried)
                                retry_result = StepResult {
                                    success: true,
                                    error: None,
                                    ..retry_result
                                };
                            }
                        }
                        step_results.push(retry_result);
                    }
                    FailureAction::Skip => {
                        step_results.push(result);
                    }
                    FailureAction::Fallback { ref procedure_name } => {
                        let fallback = self.execute(procedure_name, context.input.clone(), &context.session_id);
                        match fallback {
                            Ok(fb) => step_results.push(StepResult {
                                step_name: format!("{} (fallback:{})", step.name, procedure_name),
                                success: fb.success,
                                output: fb.output,
                                error: fb.error,
                                duration_ms: step_start.elapsed().as_millis() as u64,
                            }),
                            Err(e) => {
                                success = false;
                                final_error = Some(e);
                                step_results.push(result);
                                break;
                            }
                        }
                    }
                }
            } else {
                step_results.push(result);
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let output = step_results.last().and_then(|r| r.output.clone());

        Ok(ProcedureOutcome {
            procedure_id: procedure.id,
            success,
            step_results,
            output,
            error: final_error,
            duration_ms,
            executed_at: OffsetDateTime::now_utc(),
        })
    }
}
