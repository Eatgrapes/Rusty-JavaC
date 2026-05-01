use crate::config::CompilerConfig;

pub struct Compiler {
    config: CompilerConfig,
}

impl Compiler {
    pub fn new(config: CompilerConfig) -> Self {
        Self { config }
    }

    pub fn compile(self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        for source_file in &self.config.source_files {
            match std::fs::read_to_string(source_file) {
                Ok(source) => {
                    if let Err(e) = self.compile_source(&source, source_file) {
                        errors.extend(e);
                    }
                }
                Err(e) => {
                    errors.push(format!("failed to read {}: {}", source_file, e));
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn compile_source(&self, source: &str, filename: &str) -> Result<(), Vec<String>> {
        let parse = javac_parser::Parser::parse(source);
        if !parse.errors.is_empty() {
            return Err(parse.errors.iter().map(|e| format!("{}: {}", filename, e.message)).collect());
        }
        Ok(())
    }
}