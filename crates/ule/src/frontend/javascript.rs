use crate::{Error, LanguageFrontend};
use regex::Regex;
use tordex_tdxl::kir::{KirOp, KirProgram, KirValueId};

/// JavaScript/TypeScript frontend: recognizes async `knowledge.*` calls.
///
/// Supported patterns:
///   - `await knowledge.collect("source", {where: {...}})`
///   - `const result = await graph.traverse(entity, "rel", {depth: N})`
///   - `await knowledge.classify(input)`
///   - `await knowledge.search("query", {filter: {...}})`
///   - `await knowledge.reason(input, {rules: [...]})`
///   - `knowledge.observe(data)`
///   - `knowledge.store("key", value)`
///   - `knowledge.snapshot("name")`
#[derive(Debug, Clone)]
pub struct JavaScriptFrontend;

impl Default for JavaScriptFrontend {
    fn default() -> Self {
        Self
    }
}

impl LanguageFrontend for JavaScriptFrontend {
    fn name(&self) -> &str {
        "JavaScript"
    }

    fn extensions(&self) -> &[&str] {
        &["js", "jsx", "ts", "tsx", "mjs"]
    }

    fn compile(&self, source: &str) -> Result<KirProgram, Error> {
        compile_js(source)
    }
}

#[derive(Debug)]
struct JsBuilder {
    prog: KirProgram,
    next_id: KirValueId,
    vars: std::collections::HashMap<String, KirValueId>,
}

impl JsBuilder {
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

fn compile_js(source: &str) -> Result<KirProgram, Error> {
    let mut pb = JsBuilder::new();
    let mut pipeline: Option<KirValueId> = None;

    let re_assign = Regex::new(r#"(?:const|let|var)\s+(\w+)\s*=\s*(?:await\s+)?(?:knowledge|kg|graph|ctx)\."#).unwrap();
    let re_call = Regex::new(r#"(?:await\s+)?(?:knowledge|kg|graph|ctx)\.(\w+)\s*\(([^)]*)\)"#).unwrap();
    let re_collect = Regex::new(r#"collect\s*\(\s*"([^"]*)"#).unwrap();
    let re_traverse = Regex::new(r#"traverse\s*\(\s*(\w+)\s*,\s*"([^"]*)"#).unwrap();
    let re_classify = Regex::new(r#"classify\s*\(\s*(\w+)\s*\)"#).unwrap();
    let re_search = Regex::new(r#"search\s*\(\s*"([^"]*)"#).unwrap();
    let re_reason = Regex::new(r#"reason\s*\(\s*(\w+)"#).unwrap();
    let re_observe = Regex::new(r#"observe\s*\(\s*(\w+)\s*\)"#).unwrap();
    let re_store = Regex::new(r#"store\s*\(\s*"([^"]*)"\s*,"#).unwrap();
    let re_snapshot = Regex::new(r#"snapshot\s*\(\s*"([^"]*)"\s*\)"#).unwrap();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        let var_name = if let Some(caps) = re_assign.captures(trimmed) {
            Some(caps[1].to_string())
        } else {
            None
        };

        if let Some(caps) = re_call.captures(trimmed) {
            let call = &caps[1];
            let args = &caps[2];

            match call {
                "collect" => {
                    if let Some(inner) = re_collect.captures(trimmed) {
                        let src_id = pb.const_str(&inner[1]);
                        let out = pb.alloc();
                        let _ = pb.push_op(KirOp::Collect {
                            output: out,
                            source_id: src_id,
                        });

                        // Check for options object with where/query
                        if args.contains("where") || args.contains("filter") || args.contains("query") {
                            let filter = extract_js_filter(args);
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
                        pb.push_op(KirOp::SourceSearch {
                            output: out,
                            query_id: qid,
                        });
                        pipeline = Some(out);
                        if let Some(vn) = var_name {
                            pb.set_var(&vn, pipeline.unwrap());
                        }
                    }
                }

                "traverse" => {
                    if let Some(inner) = re_traverse.captures(trimmed) {
                        let entity = pb.get_var(&inner[1]).unwrap_or(0);
                        let rel_id = pb.const_str(&inner[2]);
                        let depth = if args.contains("depth") {
                            let re_depth = Regex::new(r#"depth\s*:\s*(\d+)"#).unwrap();
                            re_depth.captures(args).and_then(|c| c[1].parse::<i32>().ok())
                        } else {
                            None
                        };
                        let out = pb.alloc();
                        pb.push_op(KirOp::Traverse {
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
                    let input = if let Some(inner) = re_classify.captures(trimmed) {
                        pb.get_var(&inner[1]).unwrap_or(0)
                    } else {
                        pipeline.unwrap_or(0)
                    };
                    let out = pb.alloc();
                    pb.push_op(KirOp::Classify {
                        output: out,
                        input,
                    });
                    pipeline = Some(out);
                    if let Some(vn) = var_name {
                        pb.set_var(&vn, pipeline.unwrap());
                    }
                }

                "reason" | "infer" => {
                    let input = if let Some(inner) = re_reason.captures(trimmed) {
                        pb.get_var(&inner[1]).unwrap_or(0)
                    } else {
                        pipeline.unwrap_or(0)
                    };
                    let rule = pb.const_str("default_rule");
                    let out = pb.alloc();
                    pb.push_op(KirOp::Infer {
                        output: out,
                        input,
                        rule_id: rule,
                    });
                    pipeline = Some(out);
                    if let Some(vn) = var_name {
                        pb.set_var(&vn, pipeline.unwrap());
                    }
                }

                "observe" => {
                    let input = if let Some(inner) = re_observe.captures(trimmed) {
                        pb.get_var(&inner[1]).unwrap_or(0)
                    } else {
                        pipeline.unwrap_or(0)
                    };
                    let out = pb.alloc();
                    pb.push_op(KirOp::Observe {
                        output: out,
                        input,
                    });
                    pipeline = Some(out);
                    if let Some(vn) = var_name {
                        pb.set_var(&vn, pipeline.unwrap());
                    }
                }

                "store" => {
                    if let Some(inner) = re_store.captures(trimmed) {
                        let name_id = pb.const_str(&inner[1]);
                        let value_id = pipeline.unwrap_or(0);
                        pb.push_op(KirOp::Store { value_id, name_id });
                    }
                }

                "snapshot" => {
                    if let Some(_inner) = re_snapshot.captures(trimmed) {
                        let val_id = pipeline.unwrap_or(0);
                        let out = pb.alloc();
                        pb.push_op(KirOp::Snapshot {
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

    if let Some(last) = pipeline {
        pb.prog.push(KirOp::Return { value_id: last });
        pb.prog.output = Some(last);
    }

    pb.prog.next_id = pb.next_id;
    Ok(pb.prog)
}

fn extract_js_filter(args: &str) -> Option<tordex_tdxl::kir::FilterCond> {
    // Look for JS object patterns like {where: {field: "Status", op: "eq", value: "Alive"}}
    // First try to find a JSON-like object
    if let Some(_pos) = args.find(|c| c == '{' || c == ':') {
        // Try to find the inner object after where/filter/query key
        for key in &["where:", "filter:", "query:"] {
            if let Some(p) = args.find(key) {
                let start = p + key.len();
                let rest = &args[start..];
                if let Some(json_start) = rest.find('{') {
                    let json_str = &rest[json_start..];
                    if let Some(end) = json_str.find('}') {
                        let obj_str = &json_str[..=end];
                        // Replace single quotes with double quotes for JSON parsing
                        let json_clean = obj_str.replace('\'', "\"");
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_clean) {
                            if let serde_json::Value::Object(map) = &v {
                                return Some(tordex_tdxl::kir::FilterCond {
                                    field: map.get("field").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    op: map.get("op").and_then(|v| v.as_str()).unwrap_or("eq").to_string(),
                                    value: map.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_collect() {
        let source = r#"
const services = await knowledge.collect("OnionServices");
"#;
        let prog = compile_js(source).unwrap();
        assert_eq!(prog.ops.len(), 3);
    }

    #[test]
    fn test_js_traverse() {
        let source = r#"
const services = await knowledge.collect("Threats");
const related = await knowledge.traverse(services, "RELATED_TO", { depth: 2 });
"#;
        let prog = compile_js(source).unwrap();
        assert!(prog.ops.len() >= 5);
        let has_traverse = prog.ops.iter().any(|op| matches!(op, KirOp::Traverse { .. }));
        assert!(has_traverse);
    }

    #[test]
    fn test_js_classify() {
        let source = r#"
const data = await knowledge.collect("Sources");
knowledge.classify(data);
"#;
        let prog = compile_js(source).unwrap();
        let has_classify = prog.ops.iter().any(|op| matches!(op, KirOp::Classify { .. }));
        assert!(has_classify);
    }

    #[test]
    fn test_js_reason() {
        let source = "await knowledge.reason(input);";
        let prog = compile_js(source).unwrap();
        let has_infer = prog.ops.iter().any(|op| matches!(op, KirOp::Infer { .. }));
        assert!(has_infer);
    }
}
