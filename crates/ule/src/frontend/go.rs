use crate::{Error, LanguageFrontend};
use regex::Regex;
use tordex_tdxl::kir::{KirOp, KirProgram, KirValueId};

/// Go frontend: recognizes `knowledge.*` interface-based function calls.
///
/// Supported patterns:
///   - `knowledge.Collect("source")`
///   - `entity.Traverse("RELATION", depth)` / `knowledge.Traverse(entity, "rel", depth)`
///   - `knowledge.Classify(entity)`
///   - `knowledge.Search("query", filter)`
///   - `knowledge.Reason(entity, rules)`
///   - `knowledge.Correlate(a, b)`
///   - `knowledge.Observe(data)`
///   - `knowledge.Store("key", value)`
///   - `knowledge.Snapshot("name")`
///   - `knowledge.Infer(entity, rule)`
#[derive(Debug, Clone)]
pub struct GoFrontend;

impl Default for GoFrontend {
    fn default() -> Self {
        Self
    }
}

impl LanguageFrontend for GoFrontend {
    fn name(&self) -> &str {
        "Go"
    }

    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn compile(&self, source: &str) -> Result<KirProgram, Error> {
        compile_go(source)
    }
}

#[derive(Debug)]
struct GoBuilder {
    prog: KirProgram,
    next_id: KirValueId,
    vars: std::collections::HashMap<String, KirValueId>,
}

impl GoBuilder {
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

fn compile_go(source: &str) -> Result<KirProgram, Error> {
    let mut pb = GoBuilder::new();
    let mut pipeline: Option<KirValueId> = None;

    let re_assign = Regex::new(r#"(\w+)\s*[_:]?=\s*(?:knowledge|kg|graph)\."#).unwrap();
    let re_collect = Regex::new(r#"(?:knowledge|kg|graph)\.Collect\s*\(\s*"([^"]*)"\s*\)"#).unwrap();
    let re_traverse = Regex::new(r#"([\w.]+)\.Traverse\s*\(\s*"([^"]*)"\s*"#).unwrap();
    let re_classify = Regex::new(r#"(?:knowledge|kg)\.Classify\s*\(\s*(\w+)\s*\)"#).unwrap();
    let re_search = Regex::new(r#"(?:knowledge|kg|graph)\.Search\s*\(\s*"([^"]*)"\s*"#).unwrap();
    let re_reason = Regex::new(r#"(?:knowledge|kg)\.Reason\s*\(\s*(\w+)"#).unwrap();
    let re_correlate = Regex::new(r#"(?:knowledge|kg)\.Correlate\s*\(\s*(\w+)\s*,\s*(\w+)"#).unwrap();
    let re_observe = Regex::new(r#"(?:knowledge|kg)\.Observe\s*\(\s*(\w+)\s*\)"#).unwrap();
    let re_store = Regex::new(r#"(?:knowledge|kg)\.Store\s*\(\s*"([^"]*)"\s*,"#).unwrap();
    let re_snapshot = Regex::new(r#"(?:knowledge|kg)\.Snapshot\s*\(\s*"([^"]*)"\s*\)"#).unwrap();
    let re_infer = Regex::new(r#"(?:knowledge|kg)\.Infer\s*\(\s*(\w+)"#).unwrap();

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

        if let Some(caps) = re_collect.captures(trimmed) {
            let src_id = pb.const_str(&caps[1]);
            let out = pb.alloc();
            pb.push_op(KirOp::Collect {
                output: out,
                source_id: src_id,
            });
            pipeline = Some(out);
            if let Some(vn) = var_name {
                pb.set_var(&vn, pipeline.unwrap());
            }
        } else if let Some(caps) = re_traverse.captures(trimmed) {
            let entity = pb.get_var(&caps[1]).unwrap_or(0);
            let rel_id = pb.const_str(&caps[2]);

            // Look for depth in remaining args
            let rest = &trimmed[caps.get(0).map(|m| m.end()).unwrap_or(0)..];
            let depth = Regex::new(r#"(\d+)\s*\)"#).unwrap()
                .captures(rest)
                .and_then(|d| d[1].parse::<i32>().ok());

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
        } else if let Some(caps) = re_classify.captures(trimmed) {
            let input = pb.get_var(&caps[1]).unwrap_or(0);
            let out = pb.alloc();
            pb.push_op(KirOp::Classify {
                output: out,
                input,
            });
            pipeline = Some(out);
            if let Some(vn) = var_name {
                pb.set_var(&vn, pipeline.unwrap());
            }
        } else if let Some(caps) = re_search.captures(trimmed) {
            let qid = pb.const_str(&caps[1]);
            let out = pb.alloc();
            pb.push_op(KirOp::SourceSearch {
                output: out,
                query_id: qid,
            });
            pipeline = Some(out);
            if let Some(vn) = var_name {
                pb.set_var(&vn, pipeline.unwrap());
            }
        } else if let Some(caps) = re_reason.captures(trimmed) {
            let input = pb.get_var(&caps[1]).unwrap_or(0);
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
        } else if let Some(caps) = re_correlate.captures(trimmed) {
            let entity = pb.get_var(&caps[1]).unwrap_or(0);
            let target = pb.get_var(&caps[2]).unwrap_or(0);
            let out = pb.alloc();
            pb.push_op(KirOp::Correlate {
                output: out,
                input: entity,
                target_id: target,
            });
            pipeline = Some(out);
            if let Some(vn) = var_name {
                pb.set_var(&vn, pipeline.unwrap());
            }
        } else if let Some(caps) = re_observe.captures(trimmed) {
            let input = pb.get_var(&caps[1]).unwrap_or(0);
            let out = pb.alloc();
            pb.push_op(KirOp::Observe {
                output: out,
                input,
            });
            pipeline = Some(out);
            if let Some(vn) = var_name {
                pb.set_var(&vn, pipeline.unwrap());
            }
        } else if let Some(caps) = re_store.captures(trimmed) {
            let name_id = pb.const_str(&caps[1]);
            let value_id = pipeline.unwrap_or(0);
            pb.push_op(KirOp::Store { value_id, name_id });
        } else if let Some(_caps) = re_snapshot.captures(trimmed) {
            let val_id = pipeline.unwrap_or(0);
            let out = pb.alloc();
            pb.push_op(KirOp::Snapshot {
                output: out,
                value_id: val_id,
                kind: 0,
            });
            pipeline = Some(out);
            if let Some(vn) = var_name {
                pb.set_var(&vn, pipeline.unwrap());
            }
        } else if let Some(caps) = re_infer.captures(trimmed) {
            let input = pb.get_var(&caps[1]).unwrap_or(0);
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
    }

    if let Some(last) = pipeline {
        pb.prog.push(KirOp::Return { value_id: last });
        pb.prog.output = Some(last);
    }

    pb.prog.next_id = pb.next_id;
    Ok(pb.prog)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_collect() {
        let source = r#"
services := knowledge.Collect("OnionServices")
"#;
        let prog = compile_go(source).unwrap();
        assert_eq!(prog.ops.len(), 3);
    }

    #[test]
    fn test_go_traverse() {
        let source = r#"
related := entity.Traverse("LINKS_TO", 3)
"#;
        let prog = compile_go(source).unwrap();
        let has_traverse = prog.ops.iter().any(|op| matches!(op, KirOp::Traverse { .. }));
        assert!(has_traverse);
    }

    #[test]
    fn test_go_classify() {
        let source = r#"
result := kg.Classify(entity)
"#;
        let prog = compile_go(source).unwrap();
        let has_classify = prog.ops.iter().any(|op| matches!(op, KirOp::Classify { .. }));
        assert!(has_classify);
    }

    #[test]
    fn test_go_full_pipeline() {
        let source = r#"
services := knowledge.Collect("OnionServices")
result := knowledge.Classify(services)
"#;
        let prog = compile_go(source).unwrap();
        assert!(prog.ops.len() >= 4);
        let has_return = prog.ops.iter().any(|op| matches!(op, KirOp::Return { .. }));
        assert!(has_return);
    }
}
