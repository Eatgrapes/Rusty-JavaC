pub struct CompilerConfig {
    pub java_version: u32,
    pub output_dir: String,
    pub classpath: Vec<String>,
    pub source_files: Vec<String>,
    pub incremental: bool,
}

impl CompilerConfig {
    pub fn new() -> Self {
        Self {
            java_version: 21,
            output_dir: ".".to_string(),
            classpath: Vec::new(),
            source_files: Vec::new(),
            incremental: false,
        }
    }
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self::new()
    }
}
