use crate::{Error, LanguageFrontend};
use regex::Regex;
use tordex_tdxl::kir::{FilterCond, KirOp, KirProgram, KirValueId};

/// Python frontend: recognizes `knowledge.*` function calls and maps to KIR.
///
/// Supported patterns:
///   - `knowledge.collect("source")` / `knowledge.collect("source", where={...})`
///   - `result = knowledge.collect(...)`  (variable assignment)
///   - `knowledge.traverse(entity, "rel", depth=N)`
///   - `knowledge.classify(entity)`
///   - `knowledge.search("query", filter={...})`
///   - `knowledge.reason(entity, rules=[...])`
///   - `knowledge.correlate(entity, target)`
///   - `knowledge.similar(a, b, metric="...")`
///   - `knowledge.observe(data)`
///   - `knowledge.timeline("entity", start="...", end="...")`
///   - `knowledge.store(key="...", value=result)`
///   - `knowledge.snapshot("name")`
#[derive(Debug, Clone)]
pub struct PythonFrontend;

impl Default for PythonFrontend {
    fn default() -> Self {
        Self
    }
}

impl LanguageFrontend for PythonFrontend {
    fn name(&self) -> &str {
        "Python"
    }

    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn compile(&self, source: &str) -> Result<KirProgram, Error> {
        compile_python(source)
    }
}

#[derive(Debug)]
struct ProgramBuilder {
    prog: KirProgram,
    next_id: KirValueId,
    vars: std::collections::HashMap<String, KirValueId>,
}

impl ProgramBuilder {
    fn new() -> Self {
        Self {
            prog: KirProgram::new(),
            next_id: 0,
            vars: std::collections::HashMap::new(),
        }
    }

    fn alloc(&mut self) -> KirValueId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn const_str(&mut self, s: &str) -> KirValueId {
        let id = self.alloc();
        self.prog.push(KirOp::ConstStr {
            output: id,
            value: s.to_string(),
        });
        id
    }

    fn set_var(&mut self, name: &str, id: KirValueId) {
        self.vars.insert(name.to_string(), id);
    }

    fn get_var(&self, name: &str) -> Option<KirValueId> {
        self.vars.get(name).copied()
    }

    fn push_op(&mut self, op: KirOp) -> KirValueId {
        let out = op.output_id().unwrap_or(self.alloc());
        self.prog.push(op);
        out
    }
}

fn compile_python(source: &str) -> Result<KirProgram, Error> {
    let mut pb = ProgramBuilder::new();

    // The current pipeline value — most recent knowledge operation result.
    let mut pipeline: Option<KirValueId> = None;

    // Regex patterns for knowledge API calls.
    let re_assign = Regex::new(r#"(\w+)\s*=\s*(knowledge|kg|graph|ctx)\."#).unwrap();
    let re_call = Regex::new(r#"(knowledge|kg|graph|ctx)\.(\w+)\s*\(([^)]*)\)"#).unwrap();
    let re_collect = Regex::new(r#"collect\s*\(\s*"([^"]*)"\s*"#).unwrap();
    let re_traverse = Regex::new(r#"traverse\s*\(\s*(\w+)\s*,\s*"([^"]*)"#).unwrap();
    let re_classify = Regex::new(r#"classify\s*\(\s*(\w+)\s*\)"#).unwrap();
    let re_search = Regex::new(r#"search\s*\(\s*"([^"]*)"\s*"#).unwrap();
    let re_reason = Regex::new(r#"reason\s*\(\s*(\w+)"#).unwrap();
    let re_correlate = Regex::new(r#"correlate\s*\(\s*(\w+)\s*,\s*(\w+)"#).unwrap();
    let re_similar = Regex::new(r#"similar\s*\(\s*(\w+)\s*,\s*(\w+)"#).unwrap();
    let re_observe = Regex::new(r#"observe\s*\(\s*(\w+)\s*\)"#).unwrap();
    let re_store = Regex::new(r#"store\s*\(\s*key\s*=\s*"([^"]*)"#).unwrap();
    let re_snapshot = Regex::new(r#"snapshot\s*\(\s*"([^"]*)"\s*\)"#).unwrap();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Check for variable assignment
        let var_name = if let Some(caps) = re_assign.captures(trimmed) {
            Some(caps[1].to_string())
        } else {
            None
        };

        // Check for knowledge API call
        if let Some(caps) = re_call.captures(trimmed) {
            let call = &caps[2];
            let args = &caps[3];

            match call {
                "collect" => {
                    if let Some(inner) = re_collect.captures(trimmed) {
                        let src_id = pb.const_str(&inner[1]);
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Collect {
                            output: out,
                            source_id: src_id,
                        });

                        // Check for where filter in kwargs
                        if args.contains("where") || args.contains("filter") {
                            // Parse simple inline dict: `{"field": "Status", "op": "eq", "value": "Alive"}`
                            let filter = extract_filter(args);
                            if let Some(f) = filter {
                                let out2 = pb.alloc();
                                pb.push_op(KirOp::Filter {
                                    output: out2,
                                    input: out,
                                    filter: f,
                                });
                                pipeline = Some(out2);
                            } else {
                                pipeline = Some(out);
                            }
                        } else {
                            pipeline = Some(out);
                        }

                        if let Some(vn) = var_name {
                            pb.set_var(&vn, pipeline.unwrap());
                        }
                    }
                }

                "search" => {
                    if let Some(inner) = re_search.captures(trimmed) {
                        let qid = pb.const_str(&inner[1]);
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::SourceSearch {
                            output: out,
                            query_id: qid,
                        });
                        pipeline = Some(out);

                        // Check for filter
                        if args.contains("filter") || args.contains("where") {
                            let filter = extract_filter(args);
                            if let Some(f) = filter {
                                let out2 = pb.alloc();
                                pb.push_op(KirOp::Filter {
                                    output: out2,
                                    input: out,
                                    filter: f,
                                });
                                pipeline = Some(out2);
                            }
                        }

                        if let Some(vn) = var_name {
                            pb.set_var(&vn, pipeline.unwrap());
                        }
                    }
                }

                "traverse" => {
                    if let Some(inner) = re_traverse.captures(trimmed) {
                        let entity = pb.get_var(&inner[1]).unwrap_or(0);
                        let rel_id = pb.const_str(&inner[2]);

                        // Extract depth from kwargs
                        let depth = if args.contains("depth") {
                            extract_int_arg(args, "depth")
                        } else {
                            None
                        };

                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Traverse {
                            output: out,
                            input: entity,
                            relation_id: rel_id,
                            depth,
                        });
                        pipeline = Some(out);
                        if let Some(vn) = var_name {
                            pb.set_var(&vn, pipeline.unwrap());
                        }
                    }
                }

                "classify" => {
                    if let Some(inner) = re_classify.captures(trimmed) {
                        let entity = pb.get_var(&inner[1]).unwrap_or(0);
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Classify {
                            output: out,
                            input: entity,
                        });
                        pipeline = Some(out);
                        if let Some(vn) = var_name {
                            pb.set_var(&vn, pipeline.unwrap());
                        }
                    } else if let Some(vn) = var_name {
                        // `result = knowledge.classify()` — use pipeline as input
                        let input = pipeline.unwrap_or(0);
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Classify {
                            output: out,
                            input,
                        });
                        pipeline = Some(out);
                        pb.set_var(&vn, pipeline.unwrap());
                    }
                }

                "reason" | "infer" => {
                    if let Some(inner) = re_reason.captures(trimmed) {
                        let entity = pb.get_var(&inner[1]).unwrap_or(0);
                        let rule = pb.const_str("default_rule");
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Infer {
                            output: out,
                            input: entity,
                            rule_id: rule,
                        });
                        pipeline = Some(out);
                        if let Some(vn) = var_name {
                            pb.set_var(&vn, pipeline.unwrap());
                        }
                    }
                }

                "correlate" => {
                    if let Some(inner) = re_correlate.captures(trimmed) {
                        let entity = pb.get_var(&inner[1]).unwrap_or(0);
                        let target = pb.get_var(&inner[2]).unwrap_or(0);
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Correlate {
                            output: out,
                            input: entity,
                            target_id: target,
                        });
                        pipeline = Some(out);
                        if let Some(vn) = var_name {
                            pb.set_var(&vn, pipeline.unwrap());
                        }
                    }
                }

                "similar" | "similarity" => {
                    if let Some(inner) = re_similar.captures(trimmed) {
                        let target = pb.get_var(&inner[1]).unwrap_or(0);
                        // second arg is the comparison target
                        let _other = pb.get_var(&inner[2]).unwrap_or(0);
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Similar {
                            output: out,
                            target_id: target,
                            threshold: 0.75,
                        });
                        pipeline = Some(out);
                        if let Some(vn) = var_name {
                            pb.set_var(&vn, pipeline.unwrap());
                        }
                    }
                }

                "observe" => {
                    if let Some(inner) = re_observe.captures(trimmed) {
                        let input = pb.get_var(&inner[1]).unwrap_or(0);
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Observe {
                            output: out,
                            input,
                        });
                        pipeline = Some(out);
                        if let Some(vn) = var_name {
                            pb.set_var(&vn, pipeline.unwrap());
                        }
                    }
                }

                "store" => {
                    if let Some(inner) = re_store.captures(trimmed) {
                        let name_id = pb.const_str(&inner[1]);
                        let value_id = pipeline.unwrap_or(0);
                        pb.push_op(KirOp::Store {
                            value_id,
                            name_id,
                        });
                    }
                }

                "snapshot" => {
                    if let Some(_inner) = re_snapshot.captures(trimmed) {
                        let val_id = pipeline.unwrap_or(0);
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Snapshot {
                            output: out,
                            value_id: val_id,
                            kind: 0,
                        });
                        pipeline = Some(out);
                    }
                }

                _ => {}
            }
        }
    }

    // Add Return if we have a pipeline value.
    if let Some(last) = pipeline {
        pb.prog.push(KirOp::Return { value_id: last });
        pb.prog.output = Some(last);
    }

    pb.prog.next_id = pb.next_id;
    Ok(pb.prog)
}

/// Extract a filter dict from args like `where={"field": "Status", "op": "eq", "value": "Alive"}`
fn extract_filter(args: &str) -> Option<FilterCond> {
    // Try to find a JSON-like dict after where= or filter=
    for prefix in &["where=", "filter="] {
        if let Some(pos) = args.find(prefix) {
            let start = pos + prefix.len();
            let rest = &args[start..];
            if let Some(json_start) = rest.find('{') {
                let json_str = &rest[json_start..];
                // Find matching closing brace
                let mut depth = 0u32;
                let end = json_str
                    .char_indices()
                    .find(|(_, c)| {
                        match c {
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            _ => {}
                        }
                        depth == 0
                    })
                    .map(|(i, _)| i + 1)
                    .unwrap_or(json_str.len());
                let obj_str = &json_str[..end];
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(obj_str) {
                    if let serde_json::Value::Object(map) = &v {
                        return Some(FilterCond {
                            field: map.get("field").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            op: map.get("op").and_then(|v| v.as_str()).unwrap_or("eq").to_string(),
                            value: map.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        });
                    }
                }
            }
        }
    }
    None
}

/// Extract an integer named argument from the args string.
fn extract_int_arg(args: &str, name: &str) -> Option<i32> {
    let re = Regex::new(&format!(r#"{}\s*=\s*(\d+)"#, regex::escape(name))).unwrap();
    re.captures(args)
        .and_then(|c| c[1].parse::<i32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_collect() {
        let source = r#"
import knowledge_graph as kg
services = kg.collect("OnionServices")
"#;
        let prog = compile_python(source).unwrap();
        assert_eq!(prog.ops.len(), 3); // ConstStr + Collect + Return
        assert!(prog.output.is_some());
    }

    #[test]
    fn test_python_collect_with_filter() {
        let source = r#"
knowledge.collect("OnionServices", where={"field": "Status", "op": "eq", "value": "Alive"})
"#;
        let prog = compile_python(source).unwrap();
        assert_eq!(prog.ops.len(), 4); // ConstStr + Collect + Filter + Return
    }

    #[test]
    fn test_python_traverse() {
        let source = r#"
services = knowledge.collect("OnionServices")
related = knowledge.traverse(services, "LINKS_TO", depth=3)
"#;
        let prog = compile_python(source).unwrap();
        assert_eq!(prog.ops.len(), 5); // ConstStr + Collect + ConstStr + Traverse + Return
    }

    #[test]
    fn test_python_classify() {
        let source = r#"
data = knowledge.collect("Sources")
knowledge.classify(data)
"#;
        let prog = compile_python(source).unwrap();
        assert!(prog.ops.len() >= 3); // Contains classify
        let has_classify = prog.ops.iter().any(|op| matches!(op, KirOp::Classify { .. }));
        assert!(has_classify);
    }

    #[test]
    fn test_python_full_pipeline() {
        let source = r#"
# Collect onion services
services = knowledge.collect("OnionServices", where={"field": "Status", "op": "eq", "value": "Alive"})
# Classify each
knowledge.classify(services)
"#;
        let prog = compile_python(source).unwrap();
        assert!(prog.ops.len() >= 4);
        let has_return = prog.ops.iter().any(|op| matches!(op, KirOp::Return { .. }));
        assert!(has_return);
    }
}
