pub mod ast;
pub mod bytecode;
pub mod call_resolver;
pub mod classfile;
pub mod compiler;
pub mod diagnostics;
pub mod hir;
pub mod lexer;
pub mod parser;
pub mod ty;

pub use compiler::config::CompilerConfig;
pub use compiler::pipeline::compile;
