use serde::{Deserialize, Serialize};

use tordex_types::{Knowledge, Relationship};

/// The value that a VM register can hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegisterValue {
    Int(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Knowledge(Knowledge),
    Relationship(Relationship),
    Entity { kind: String, id: String },
    Facts(Vec<RegisterValue>),
    Nil,
}

impl RegisterValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Int(v) => *v != 0,
            Self::Float(v) => *v != 0.0,
            Self::Boolean(b) => *b,
            Self::String(s) => !s.is_empty(),
            Self::Knowledge(_) => true,
            Self::Relationship(_) => true,
            Self::Entity { .. } => true,
            Self::Facts(v) => !v.is_empty(),
            Self::Nil => false,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Float(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Int(v) => serde_json::json!(*v),
            Self::Float(v) => serde_json::json!(*v),
            Self::Boolean(b) => serde_json::json!(*b),
            Self::String(s) => serde_json::json!(s),
            Self::Knowledge(k) => serde_json::to_value(k).unwrap_or_default(),
            Self::Relationship(r) => serde_json::to_value(r).unwrap_or_default(),
            Self::Entity { kind, id } => serde_json::json!({"kind": kind, "id": id}),
            Self::Facts(v) => serde_json::json!(v),
            Self::Nil => serde_json::Value::Null,
        }
    }
}

impl From<i64> for RegisterValue {
    fn from(v: i64) -> Self { Self::Int(v) }
}

impl From<f64> for RegisterValue {
    fn from(v: f64) -> Self { Self::Float(v) }
}

impl From<bool> for RegisterValue {
    fn from(v: bool) -> Self { Self::Boolean(v) }
}

impl From<String> for RegisterValue {
    fn from(v: String) -> Self { Self::String(v) }
}

impl From<&str> for RegisterValue {
    fn from(v: &str) -> Self { Self::String(v.to_string()) }
}

/// The register file — 32 typed registers.
///
/// - R0–R15: general purpose (16)
/// - R16–R23: knowledge registers (8)
/// - R24–R27: string/temporary registers (4)
/// - R28–R31: reserved (frame pointer, etc.)
#[derive(Debug, Clone)]
pub struct RegisterFile {
    regs: [RegisterValue; 32],
}

impl Default for RegisterFile {
    fn default() -> Self {
        Self::new()
    }
}

impl RegisterFile {
    pub fn new() -> Self {
        Self {
            regs: array_init(),
        }
    }

    pub fn read(&self, idx: u8) -> Result<&RegisterValue, RegisterError> {
        self.regs
            .get(idx as usize)
            .ok_or(RegisterError::InvalidRegister(idx))
    }

    pub fn write(&mut self, idx: u8, value: RegisterValue) -> Result<(), RegisterError> {
        let slot = self
            .regs
            .get_mut(idx as usize)
            .ok_or(RegisterError::InvalidRegister(idx))?;
        *slot = value;
        Ok(())
    }

    pub fn clear(&mut self) {
        for r in &mut self.regs {
            *r = RegisterValue::Nil;
        }
    }
}

fn array_init() -> [RegisterValue; 32] {
    [
        RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil,
        RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil,
        RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil,
        RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil,
        RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil,
        RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil,
        RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil,
        RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil, RegisterValue::Nil,
    ]
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RegisterError {
    #[error("invalid register index: {0}")]
    InvalidRegister(u8),
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeError { expected: &'static str, actual: &'static str },
}

impl From<RegisterError> for String {
    fn from(e: RegisterError) -> Self {
        e.to_string()
    }
}
