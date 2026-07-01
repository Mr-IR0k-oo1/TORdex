pub mod ast;
pub mod compiler;
pub mod error;
pub mod kir;
pub mod lexer;
pub mod optimizer;
pub mod parser;
pub mod query_optimizer;
pub mod token;

pub use compiler::compile;
pub use error::Error;

/// Parse and compile a TDXL source string into a VM Program.
pub fn compile_program(source: &str) -> Result<tordex_vm::Program, Error> {
    compiler::compile(source)
}
