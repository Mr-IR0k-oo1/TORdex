use serde::{Deserialize, Serialize};

use crate::instruction::Instruction;

/// A VM program — a sequence of intelligence bytecode instructions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub name: String,
    pub version: String,
    pub instructions: Vec<Instruction>,
    pub constants: Vec<serde_json::Value>,
    pub entry_point: usize,
    pub metadata: std::collections::HashMap<String, String>,
}

impl Program {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            instructions: Vec::new(),
            constants: Vec::new(),
            entry_point: 0,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub fn push(&mut self, instr: Instruction) {
        self.instructions.push(instr);
    }

    pub fn add_constant(&mut self, value: serde_json::Value) -> usize {
        let idx = self.constants.len();
        self.constants.push(value);
        idx
    }

    pub fn get_constant(&self, idx: u16) -> Option<&serde_json::Value> {
        self.constants.get(idx as usize)
    }

    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Encode the program as raw bytes.
    pub fn encode(&self) -> Vec<u8> {
        let header_size = 16;
        let instr_size = self.instructions.len() * 8;
        let const_data: Vec<u8> = self
            .constants
            .iter()
            .flat_map(|c| serde_json::to_vec(c).unwrap_or_default())
            .collect();
        let mut buf = Vec::with_capacity(header_size + instr_size + const_data.len());

        // Header: magic + count + entry + const_size
        buf.extend_from_slice(b"TVM\x00"); // magic
        buf.extend_from_slice(&(self.instructions.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(self.entry_point as u32).to_le_bytes());
        buf.extend_from_slice(&(const_data.len() as u32).to_le_bytes());

        // Instructions
        for instr in &self.instructions {
            buf.extend_from_slice(&instr.encode());
        }

        // Constants
        buf.extend_from_slice(&const_data);

        buf
    }
}
