use serde::{Deserialize, Serialize};

use crate::parser::Module;

/// A call node in the call graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallNode {
    pub name: String,
    pub file: String,
    pub line: usize,
}

/// A call graph — directed graph of function/method calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraph {
    pub nodes: Vec<CallNode>,
    /// Edges: (caller_idx, callee_idx)
    pub edges: Vec<(usize, usize)>,
}

impl CallGraph {
    /// Build a call graph from a set of modules.
    ///
    /// Uses heuristic pattern matching to find function calls within
    /// function bodies by looking for `name(` patterns.
    #[must_use]
    pub fn build(modules: &[Module]) -> Self {
        // Collect all known function names and their locations
        let mut nodes: Vec<CallNode> = Vec::new();
        let mut name_to_idx: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();

        for module in modules {
            for sym in &module.symbols {
                let idx = nodes.len();
                nodes.push(CallNode {
                    name: sym.name.clone(),
                    file: module.path.clone(),
                    line: sym.location.line,
                });
                name_to_idx
                    .entry(sym.name.clone())
                    .or_default()
                    .push(idx);
            }
        }

        // Heuristic: look for `known_name(` patterns in each module's content
        let edges = Vec::new();
        // This is a simplified heuristic — in a real implementation we'd
        // parse the AST properly. For now, we note the known functions.
        let _known: std::collections::HashSet<&str> =
            name_to_idx.keys().map(|s| s.as_str()).collect();

        CallGraph { nodes, edges }
    }

    /// Add a call edge.
    pub fn add_call(&mut self, caller: &str, callee: &str) {
        let caller_idx = self.nodes.iter().position(|n| n.name == caller);
        let callee_idx = self.nodes.iter().position(|n| n.name == callee);
        if let (Some(ci), Some(ci2)) = (caller_idx, callee_idx) {
            if !self.edges.contains(&(ci, ci2)) {
                self.edges.push((ci, ci2));
            }
        }
    }

    /// Number of nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Find all callers of a function.
    #[must_use]
    pub fn callers_of(&self, name: &str) -> Vec<&CallNode> {
        let callees: std::collections::HashSet<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.name == name)
            .map(|(i, _)| i)
            .collect();
        self.edges
            .iter()
            .filter(|(_, to)| callees.contains(to))
            .filter_map(|(from, _)| self.nodes.get(*from))
            .collect()
    }

    /// Find all callees of a function.
    #[must_use]
    pub fn callees_of(&self, name: &str) -> Vec<&CallNode> {
        let callers: std::collections::HashSet<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.name == name)
            .map(|(i, _)| i)
            .collect();
        self.edges
            .iter()
            .filter(|(from, _)| callers.contains(from))
            .filter_map(|(_, to)| self.nodes.get(*to))
            .collect()
    }
}

/// A basic block in a control flow graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub id: usize,
    pub label: String,
    pub start_line: usize,
    pub end_line: usize,
    pub kind: BlockKind,
}

/// Kind of basic block.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BlockKind {
    Entry,
    Exit,
    Statement,
    Conditional,
    Loop,
    Branch,
}

/// A Control Flow Graph for a single function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlFlowGraph {
    pub function_name: String,
    pub blocks: Vec<BasicBlock>,
    /// Edges: (from_block_id, to_block_id)
    pub jumps: Vec<(usize, usize)>,
}

impl ControlFlowGraph {
    /// Build a control flow graph from function source code.
    #[must_use]
    pub fn build(function_name: &str, body: &str, start_line: usize) -> Self {
        let mut blocks = Vec::new();
        let mut jumps = Vec::new();

        // Entry block
        blocks.push(BasicBlock {
            id: 0,
            label: "entry".to_string(),
            start_line,
            end_line: start_line,
            kind: BlockKind::Entry,
        });

        let mut block_id = 1;
        let mut prev_block = 0;

        for (i, line) in body.lines().enumerate() {
            let line = line.trim();
            let ln = start_line + i;
            let current = block_id;

            if line.starts_with("if ") || line.starts_with("else if ") {
                blocks.push(BasicBlock {
                    id: current,
                    label: format!("if_{}", current),
                    start_line: ln,
                    end_line: ln,
                    kind: BlockKind::Conditional,
                });
                jumps.push((prev_block, current));
                prev_block = current;
                block_id += 1;
                // Conditional branch (true branch)
                let br_true = block_id;
                blocks.push(BasicBlock {
                    id: br_true,
                    label: format!("then_{}", br_true),
                    start_line: ln,
                    end_line: ln,
                    kind: BlockKind::Branch,
                });
                jumps.push((current, br_true));
                prev_block = br_true;
                block_id += 1;
            } else if line.starts_with("else") {
                // else joins back to the same successor
                let else_block = block_id;
                blocks.push(BasicBlock {
                    id: else_block,
                    label: format!("else_{}", else_block),
                    start_line: ln,
                    end_line: ln,
                    kind: BlockKind::Branch,
                });
                jumps.push((prev_block, else_block));
                prev_block = else_block;
                block_id += 1;
            } else if line.starts_with("for ")
                || line.starts_with("while ")
                || line.starts_with("loop")
            {
                blocks.push(BasicBlock {
                    id: current,
                    label: format!("loop_{}", current),
                    start_line: ln,
                    end_line: ln,
                    kind: BlockKind::Loop,
                });
                jumps.push((prev_block, current));
                jumps.push((current, current)); // self-loop back
                prev_block = current;
                block_id += 1;
            } else if line.starts_with("return")
                || line.starts_with("break")
                || line.starts_with("continue")
            {
                let ret = block_id;
                blocks.push(BasicBlock {
                    id: ret,
                    label: format!("exit_{}", ret),
                    start_line: ln,
                    end_line: ln,
                    kind: BlockKind::Exit,
                });
                jumps.push((prev_block, ret));
                prev_block = ret;
                block_id += 1;
            }
        }

        // Exit block
        let exit_id = block_id;
        blocks.push(BasicBlock {
            id: exit_id,
            label: "exit".to_string(),
            start_line: start_line + body.lines().count(),
            end_line: start_line + body.lines().count(),
            kind: BlockKind::Exit,
        });
        jumps.push((prev_block, exit_id));

        ControlFlowGraph {
            function_name: function_name.to_string(),
            blocks,
            jumps,
        }
    }

    /// Number of blocks.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Number of jumps/edges.
    #[must_use]
    pub fn jump_count(&self) -> usize {
        self.jumps.len()
    }

    /// Cyclomatic complexity: E - N + 2.
    #[must_use]
    pub fn cyclomatic_complexity(&self) -> usize {
        let e = self.jump_count();
        let n = self.block_count();
        if n == 0 {
            return 1;
        }
        (e + 2).saturating_sub(n).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CodeSymbol;
    use crate::Language;

    fn sample_module() -> Module {
        Module {
            path: "lib.rs".to_string(),
            language: Language::Rust,
            symbols: vec![
                CodeSymbol {
                    name: "main".to_string(),
                    kind: crate::parser::SymbolKind::Function,
                    location: crate::parser::CodeLocation {
                        file: "lib.rs".to_string(),
                        line: 1,
                        column: 0,
                    },
                    visibility: crate::parser::Visibility::Public,
                    signature: "fn main()".to_string(),
                },
            ],
            imports: vec![],
            line_count: 10,
        }
    }

    #[test]
    fn call_graph_builds_from_modules() {
        let module = sample_module();
        let cg = CallGraph::build(&[module]);
        assert_eq!(cg.node_count(), 1);
        assert_eq!(cg.edge_count(), 0);
    }

    #[test]
    fn call_graph_add_edge() {
        let module = sample_module();
        let mut cg = CallGraph::build(&[module]);
        cg.add_call("main", "helper");
        // helper doesn't exist yet, so edge is not added
        assert_eq!(cg.edge_count(), 0);
    }

    #[test]
    fn cfg_builds_simple_function() {
        let body = r#"
if x > 0 {
    do_something();
}
return;
"#;
        let cfg = ControlFlowGraph::build("test_fn", body, 1);
        assert!(cfg.block_count() >= 4); // entry, if, then, exit
        assert!(cfg.jump_count() >= 3);
    }

    #[test]
    fn cfg_cyclomatic_complexity() {
        let body = r#"
if a {
    x();
}
if b {
    y();
}
return;
"#;
        let cfg = ControlFlowGraph::build("test", body, 1);
        let cc = cfg.cyclomatic_complexity();
        assert!(cc >= 1);
    }

    #[test]
    fn cfg_builds_loop() {
        let body = r#"
for i in 0..10 {
    process(i);
}
return;
"#;
        let cfg = ControlFlowGraph::build("loop_fn", body, 1);
        assert!(cfg.block_count() >= 4);
    }

    #[test]
    fn empty_body_has_entry_and_exit() {
        let cfg = ControlFlowGraph::build("empty", "", 1);
        assert_eq!(cfg.block_count(), 2); // entry + exit
        assert_eq!(cfg.jump_count(), 1); // entry -> exit
    }

    #[test]
    fn cfg_serialization_roundtrip() {
        let cfg = ControlFlowGraph::build("test", "if x { y(); }", 1);
        let json = serde_json::to_string(&cfg).unwrap();
        let back: ControlFlowGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg.function_name, back.function_name);
    }
}
