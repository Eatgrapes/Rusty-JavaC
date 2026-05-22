use crate::config::CompilerConfig;
use javac_ast::JavaSyntaxNode;
use javac_hir::hir::CompilationUnit;
use std::path::{Path, PathBuf};

type CompileResult<T> = Result<T, Vec<String>>;

pub struct Compiler {
    config: CompilerConfig,
}

struct ClassArtifact {
    internal_name: String,
    bytes: Vec<u8>,
}

impl Compiler {
    pub fn new(config: CompilerConfig) -> Self {
        Self { config }
    }

    pub fn compile(self) -> CompileResult<()> {
        let mut errors = Vec::new();
        for source_file in &self.config.source_files {
            if let Err(error) = self.compile_file(source_file) {
                errors.extend(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn compile_file(&self, source_file: &str) -> CompileResult<()> {
        let source = read_source_file(source_file)?;
        let artifact = compile_source(source_file, &source)?;
        write_class_file(&self.config.output_dir, &artifact)?;
        Ok(())
    }
}

fn read_source_file(path: &str) -> CompileResult<String> {
    std::fs::read_to_string(path).map_err(|e| vec![format!("failed to read {}: {}", path, e)])
}

fn compile_source(filename: &str, source: &str) -> CompileResult<ClassArtifact> {
    let unit = parse_and_lower(filename, source)?;
    let internal_name = top_level_class_name(filename, &unit)?;
    let bytes = javac_bytecode::class_gen::gen_class(&unit)
        .map_err(|e| vec![format!("{}: {}", filename, e)])?;

    Ok(ClassArtifact {
        internal_name,
        bytes,
    })
}

fn parse_and_lower(filename: &str, source: &str) -> CompileResult<CompilationUnit> {
    let parse = javac_parser::Parser::parse(source);
    if !parse.errors.is_empty() {
        return Err(parse
            .errors
            .iter()
            .map(|e| format!("{}:{}: {}", filename, e.offset, e.message))
            .collect());
    }

    let root = JavaSyntaxNode::new_root(parse.green_node);
    javac_hir::lowering::lower(&root).map_err(|e| vec![format!("{}: {}", filename, e)])
}

fn top_level_class_name(filename: &str, unit: &CompilationUnit) -> CompileResult<String> {
    unit.type_decls
        .first()
        .map(|decl| decl.name.to_string())
        .ok_or_else(|| vec![format!("{}: no class declaration found", filename)])
}

fn write_class_file(output_dir: &str, artifact: &ClassArtifact) -> CompileResult<()> {
    let class_path = class_file_path(output_dir, &artifact.internal_name);
    if let Some(parent) = class_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| vec![format!("failed to create {}: {}", parent.display(), e)])?;
    }
    std::fs::write(&class_path, &artifact.bytes)
        .map_err(|e| vec![format!("failed to write {}: {}", class_path.display(), e)])
}

fn class_file_path(output_dir: &str, class_name: &str) -> PathBuf {
    let mut path = Path::new(output_dir).to_path_buf();
    for segment in class_name.split('/') {
        path.push(segment);
    }
    path.set_extension("class");
    path
}
