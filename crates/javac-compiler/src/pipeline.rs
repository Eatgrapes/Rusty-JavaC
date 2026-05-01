use crate::compiler::Compiler;
use crate::config::CompilerConfig;

pub fn compile(config: CompilerConfig) -> Result<(), Vec<String>> {
    let compiler = Compiler::new(config);
    compiler.compile()
}