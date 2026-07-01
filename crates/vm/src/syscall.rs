use serde::{Deserialize, Serialize};

/// System call numbers for the VM's kernel interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum SyscallNumber {
    Log = 0,
    EmitEvent = 1,
    SpawnAgent = 2,
    SearchIndex = 3,
    DispatchTask = 4,
    GetTime = 5,
    Sleep = 6,
    ReadConfig = 7,
    WriteOutput = 8,
    InvokeProcessor = 9,
    CollectEvidence = 10,
}

impl SyscallNumber {
    pub fn from_i32(n: i32) -> Option<Self> {
        match n {
            0 => Some(Self::Log),
            1 => Some(Self::EmitEvent),
            2 => Some(Self::SpawnAgent),
            3 => Some(Self::SearchIndex),
            4 => Some(Self::DispatchTask),
            5 => Some(Self::GetTime),
            6 => Some(Self::Sleep),
            7 => Some(Self::ReadConfig),
            8 => Some(Self::WriteOutput),
            9 => Some(Self::InvokeProcessor),
            10 => Some(Self::CollectEvidence),
            _ => None,
        }
    }
}

/// Arguments for a system call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallArgs {
    pub number: SyscallNumber,
    pub arg0: i64,
    pub arg1: i64,
    pub arg2: i64,
    pub data: Option<serde_json::Value>,
}

/// Result of a system call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallResult {
    pub success: bool,
    pub value: i64,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl SyscallResult {
    pub fn ok(value: i64) -> Self {
        Self {
            success: true,
            value,
            data: None,
            error: None,
        }
    }

    pub fn ok_with_data(value: i64, data: serde_json::Value) -> Self {
        Self {
            success: true,
            value,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: &str) -> Self {
        Self {
            success: false,
            value: -1,
            data: None,
            error: Some(msg.to_string()),
        }
    }
}

/// Trait for handling VM system calls.
pub trait SyscallHandler: Send + Sync {
    fn handle(&self, args: &SyscallArgs) -> SyscallResult;
}
