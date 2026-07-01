use std::sync::Arc;

use crate::executor::{ExecutionContext, Executor};
use crate::memory::MemoryManager;
use crate::program::Program;
use crate::register::RegisterValue;
use crate::stack::InferenceStack;
use crate::syscall::SyscallHandler;

/// The TORdex Intelligence Virtual Machine.
///
/// Executes intelligence bytecode programs against memory and kernel
/// subsystems. Used by agents, processors, and the kernel itself to run
/// knowledge-oriented workloads.
pub struct IntelligenceVm {
    memory: Arc<MemoryManager>,
    syscall_handler: Arc<dyn SyscallHandler>,
}

impl IntelligenceVm {
    pub fn new(
        memory: MemoryManager,
        syscall_handler: Arc<dyn SyscallHandler>,
    ) -> Self {
        Self {
            memory: Arc::new(memory),
            syscall_handler,
        }
    }

    /// Run a program to completion.
    pub fn run(&self, program: Program) -> Result<VmResult, String> {
        let mut ctx = ExecutionContext::new(program, Arc::clone(&self.memory), Arc::clone(&self.syscall_handler));
        Executor::execute(&mut ctx)?;
        Ok(VmResult {
            cycles: ctx.cycles,
            registers: ctx.regs,
            stack: ctx.stack,
        })
    }

    /// Run a program with custom initial register values.
    pub fn run_with_args(
        &self,
        program: Program,
        args: Vec<(u8, RegisterValue)>,
    ) -> Result<VmResult, String> {
        let mut ctx = ExecutionContext::new(program, Arc::clone(&self.memory), Arc::clone(&self.syscall_handler));
        for (reg, val) in args {
            ctx.regs.write(reg, val).map_err(|e| e.to_string())?;
        }
        Executor::execute(&mut ctx)?;
        Ok(VmResult {
            cycles: ctx.cycles,
            registers: ctx.regs,
            stack: ctx.stack,
        })
    }

    /// Access the memory manager.
    pub fn memory(&self) -> &MemoryManager {
        &self.memory
    }
}

/// Result of a VM execution.
#[derive(Debug, Clone)]
pub struct VmResult {
    pub cycles: u64,
    pub registers: crate::register::RegisterFile,
    pub stack: InferenceStack,
}
