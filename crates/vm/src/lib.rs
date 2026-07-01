#![forbid(unsafe_code)]

pub mod executor;
pub mod instruction;
pub mod memory;
pub mod program;
pub mod register;
pub mod stack;
pub mod syscall;
pub mod vm;

pub use executor::{ExecutionContext, Executor, StepResult};
pub use instruction::{Instruction, Opcode};
pub use memory::MemoryManager;
pub use program::Program;
pub use register::{RegisterError, RegisterFile, RegisterValue};
pub use stack::{InferenceStack, StackError};
pub use syscall::{SyscallArgs, SyscallHandler, SyscallNumber, SyscallResult};
pub use vm::{IntelligenceVm, VmResult};
