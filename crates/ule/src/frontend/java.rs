use crate::{Error, LanguageFrontend};
use regex::Regex;
use tordex_tdxl::kir::{KirOp, KirProgram, KirValueId};

#[derive(Debug, Clone)]
pub struct JavaFrontend;

impl Default for JavaFrontend {
    fn default() -> Self {
        Self
    }
}

impl LanguageFrontend for JavaFrontend {
    fn name(&self) -> &str {
        "Java"
    }

    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn compile(&self, source: &str) -> Result<KirProgram, Error> {
        compile_java(source)
    }
}

struct JavaBuilder {
    prog: KirProgram,
    next_id: KirValueId,
    vars: std::collections::HashMap<String, KirValueId>,
}

impl JavaBuilder {
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

    fn cstr(&mut self, s: &str) -> KirValueId {
        let id = self.alloc();
        self.prog.push(KirOp::ConstStr { output: id, value: s.to_string() });
        id
    }

    fn set_var(&mut self, name: &str, id: KirValueId) {
        self.vars.insert(name.to_string(), id);
    }

    fn get_var(&self, name: &str) -> Option<KirValueId> {
        self.vars.get(name).copied()
    }
}

fn compile_java(source: &str) -> Result<KirProgram, Error> {
    let mut pb = JavaBuilder::new();
    let mut pipeline: Option<KirValueId> = None;

    let recollect = Regex::new(r#"Knowledge\.collect\s*\(\s*"([^"]*)"\s*\)"#).unwrap();
    let retraverse = Regex::new(r#"(\w+)\.traverse\s*\(\s*"([^"]*)"\s*\)"#).unwrap();
    let redepth = Regex::new(r#"\.depth\s*\(\s*(\d+)\s*\)"#).unwrap();
    let reexecute = Regex::new(r#"\.execute\s*\(\s*\)"#).unwrap();
    let reclassify = Regex::new(r#"Knowledge\.classify\s*\(\s*(\w+)\s*\)"#).unwrap();
    let research = Regex::new(r#"Knowledge\.search\s*\(\s*"([^"]*)"\s*,"#).unwrap();
    let rereson = Regex::new(r#"Knowledge\.reason\s*\(\s*(\w+)"#).unwrap();
    let reobserve = Regex::new(r#"Knowledge\.observe\s*\(\s*(\w+)\s*\)"#).unwrap();
    let restore = Regex::new(r#"Knowledge\.store\s*\(\s*"([^"]*)"\s*,"#).unwrap();
    let resnap = Regex::new(r#"Knowledge\.snapshot\s*\(\s*"([^"]*)"\s*\)"#).unwrap();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.starts_with('*') {
            continue;
        }

        // Detect `Type var =` assignments
        let var_name = {
            let parts: Vec<&str> = trimmed.splitn(3, '=').collect();
            if parts.len() >= 2 && (trimmed.contains("Knowledge.") || trimmed.contains("knowledge.")) {
                let before = parts[0].trim();
                let words: Vec<&str> = before.split_whitespace().collect();
                words.last().map(|s| s.to_string())
            } else {
                None
            }
        };

        if let Some(caps) = recollect.captures(trimmed) {
            let src_id = pb.cstr(&caps[1]);
            let out = pb.alloc();
            pb.prog.push(KirOp::Collect { output: out, source_id: src_id });
            pipeline = Some(out);
            if let Some(ref vn) = var_name {
                pb.set_var(vn, pipeline.unwrap());
            }
        } else if let Some(caps) = retraverse.captures(trimmed) {
            let entity = pb.get_var(&caps[1]).unwrap_or(0);
            let rel_id = pb.cstr(&caps[2]);
            let depth = redepth.captures(trimmed).and_then(|d| d[1].parse::<i32>().ok());
            let out = pb.alloc();
            pb.prog.push(KirOp::Traverse { output: out, input: entity, relation_id: rel_id, depth });
            pipeline = Some(out);
            if reexecute.is_match(trimmed) {
                if let Some(ref vn) = var_name {
                    pb.set_var(vn, pipeline.unwrap());
                }
            }
        } else if let Some(caps) = reclassify.captures(trimmed) {
            let input = pb.get_var(&caps[1]).unwrap_or(0);
            let out = pb.alloc();
            pb.prog.push(KirOp::Classify { output: out, input });
            pipeline = Some(out);
            if let Some(ref vn) = var_name {
                pb.set_var(vn, pipeline.unwrap());
            }
        } else if let Some(caps) = research.captures(trimmed) {
            let qid = pb.cstr(&caps[1]);
            let out = pb.alloc();
            pb.prog.push(KirOp::SourceSearch { output: out, query_id: qid });
            pipeline = Some(out);
            if let Some(ref vn) = var_name {
                pb.set_var(vn, pipeline.unwrap());
            }
        } else if let Some(caps) = rereson.captures(trimmed) {
            let input = pb.get_var(&caps[1]).unwrap_or(0);
            let rule = pb.cstr("default_rule");
            let out = pb.alloc();
            pb.prog.push(KirOp::Infer { output: out, input, rule_id: rule });
            pipeline = Some(out);
            if let Some(ref vn) = var_name {
                pb.set_var(vn, pipeline.unwrap());
            }
        } else if let Some(caps) = reobserve.captures(trimmed) {
            let input = pb.get_var(&caps[1]).unwrap_or(0);
            let out = pb.alloc();
            pb.prog.push(KirOp::Observe { output: out, input });
            pipeline = Some(out);
            if let Some(ref vn) = var_name {
                pb.set_var(vn, pipeline.unwrap());
            }
        } else if let Some(caps) = restore.captures(trimmed) {
            let name_id = pb.cstr(&caps[1]);
            let value_id = pipeline.unwrap_or(0);
            pb.prog.push(KirOp::Store { value_id, name_id });
        } else if let Some(_caps) = resnap.captures(trimmed) {
            let val_id = pipeline.unwrap_or(0);
            let out = pb.alloc();
            pb.prog.push(KirOp::Snapshot { output: out, value_id: val_id, kind: 0 });
            pipeline = Some(out);
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
    fn test_java_collect() {
        let source = "GraphResult services = Knowledge.collect(\"OnionServices\");";
        let prog = compile_java(source).unwrap();
        assert_eq!(prog.ops.len(), 3);
    }

    #[test]
    fn test_java_classify() {
        let source = "ClassificationResult result = Knowledge.classify(entity);";
        let prog = compile_java(source).unwrap();
        assert!(prog.ops.iter().any(|op| matches!(op, KirOp::Classify { .. })));
    }

    #[test]
    fn test_java_traverse() {
        let source = "GraphResult related = entity.traverse(\"RELATED_TO\").depth(3).execute();";
        let prog = compile_java(source).unwrap();
        assert!(prog.ops.iter().any(|op| matches!(op, KirOp::Traverse { .. })));
    }

    #[test]
    fn test_java_observe() {
        let source = "Knowledge.observe(data);";
        let prog = compile_java(source).unwrap();
        assert!(prog.ops.iter().any(|op| matches!(op, KirOp::Observe { .. })));
    }
}
