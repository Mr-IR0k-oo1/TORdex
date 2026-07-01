use crate::{Error, LanguageFrontend};
use regex::Regex;
use tordex_tdxl::kir::{KirOp, KirProgram, KirValueId};

/// Rust frontend: recognizes `knowledge!` macro and method-based knowledge calls.
///
/// Supported patterns:
///   - `knowledge::collect("source", |row| { ... })`
///   - `entity.traverse("RELATION")?` / `.traverse("RELATION").depth(3)?`
///   - `entity.classify()?`
///   - `knowledge::search("query", filter)?`
///   - `entity.correlate(&other)?`
///   - `entity.similar(&other)?`
///   - `knowledge::observe(&data)?`
///   - `knowledge::store("key", value)?`
///   - `knowledge::snapshot("name")?`
///   - `knowledge::infer(entity, &rules)?`
#[derive(Debug, Clone)]
pub struct RustFrontend;

impl Default for RustFrontend {
    fn default() -> Self {
        Self
    }
}

impl LanguageFrontend for RustFrontend {
    fn name(&self) -> &str {
        "Rust"
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn compile(&self, source: &str) -> Result<KirProgram, Error> {
        compile_rust(source)
    }
}

#[derive(Debug)]
struct RsBuilder {
    prog: KirProgram,
    next_id: KirValueId,
    vars: std::collections::HashMap<String, KirValueId>,
}

impl RsBuilder {
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

#[allow(clippy::single_match_else)]
fn compile_rust(source: &str) -> Result<KirProgram, Error> {
    let mut pb = RsBuilder::new();
    let mut pipeline: Option<KirValueId> = None;

    let re_let = Regex::new(r#"let\s+(mut\s+)?(\w+)\s*="#).unwrap();
    let re_collect = Regex::new(r#"knowledge::collect\s*\(\s*"([^"]*)"\s*\)"#).unwrap();
    let re_traverse_call = Regex::new(r#"(\w+)\.traverse\s*\(\s*"([^"]*)"\s*\)"#).unwrap();
    let re_traverse_depth = Regex::new(r#"\.depth\s*\(\s*(\d+)\s*\)"#).unwrap();
    let re_classify_call = Regex::new(r#"(\w+)\.classify\s*\(\s*\)"#).unwrap();
    let re_search = Regex::new(r#"knowledge::search\s*\(\s*"([^"]*)"\s*,"#).unwrap();
    let re_correlate = Regex::new(r#"(\w+)\.correlate\s*\(\s*&(\w+)"#).unwrap();
    let re_similar = Regex::new(r#"(\w+)\.similar\s*\(\s*&(\w+)"#).unwrap();
    let re_observe = Regex::new(r#"knowledge::observe\s*\(\s*&(\w+)"#).unwrap();
    let re_store = Regex::new(r#"knowledge::store\s*\(\s*"([^"]*)"\s*,"#).unwrap();
    let re_snapshot = Regex::new(r#"knowledge::snapshot\s*\(\s*"([^"]*)"\s*\)"#).unwrap();
    let re_infer = Regex::new(r#"knowledge::infer\s*\(\s*(\w+)"#).unwrap();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        let var_name = if let Some(caps) = re_let.captures(trimmed) {
            Some(caps[2].to_string())
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
        } else if let Some(caps) = re_traverse_call.captures(trimmed) {
            let entity = pb.get_var(&caps[1]).unwrap_or(0);
            let rel_id = pb.const_str(&caps[2]);
            let depth = re_traverse_depth.captures(trimmed)
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
        } else if let Some(caps) = re_classify_call.captures(trimmed) {
            let entity = pb.get_var(&caps[1]).unwrap_or(0);
            let out = pb.alloc();
            pb.push_op(KirOp::Classify {
                output: out,
                input: entity,
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
        } else if let Some(caps) = re_similar.captures(trimmed) {
            let target = pb.get_var(&caps[1]).unwrap_or(0);
            let _other = pb.get_var(&caps[2]).unwrap_or(0);
            let out = pb.alloc();
            pb.push_op(KirOp::Similar {
                output: out,
                target_id: target,
                threshold: 0.75,
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
    fn test_rust_collect() {
        let source = r#"
let services = knowledge::collect("OnionServices")?;
"#;
        let prog = compile_rust(source).unwrap();
        assert_eq!(prog.ops.len(), 3);
    }

    #[test]
    fn test_rust_traverse() {
        let source = r#"
let related = entity.traverse("LINKS_TO")?;
"#;
        let prog = compile_rust(source).unwrap();
        let has_traverse = prog.ops.iter().any(|op| matches!(op, KirOp::Traverse { .. }));
        assert!(has_traverse);
    }

    #[test]
    fn test_rust_classify() {
        let source = r#"
let categories = entity.classify()?;
"#;
        let prog = compile_rust(source).unwrap();
        let has_classify = prog.ops.iter().any(|op| matches!(op, KirOp::Classify { .. }));
        assert!(has_classify);
    }

    #[test]
    fn test_rust_full_pipeline() {
        let source = r#"
let services = knowledge::collect("OnionServices")?;
let alive = knowledge::search("Status:eq:Alive", filter)?;
let categories = services.classify()?;
"#;
        let prog = compile_rust(source).unwrap();
        assert!(prog.ops.len() >= 5);
        let has_return = prog.ops.iter().any(|op| matches!(op, KirOp::Return { .. }));
        assert!(has_return);
    }
}
