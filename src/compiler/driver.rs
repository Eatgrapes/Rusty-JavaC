use crate::ast::JavaSyntaxNode;
use crate::call_resolver::ClassCatalog;
use crate::compiler::classpath::build_class_catalog;
use crate::compiler::config::CompilerConfig;
use crate::compiler::diagnostic::{render_bytecode_error, render_lower_error};
use crate::compiler::incremental::IncrementalBuild;
use crate::diagnostics::{SourceFile, render_diagnostics};
use crate::hir::CompilationUnit;
use std::path::{Path, PathBuf};

type CompileResult<T> = Result<T, Vec<String>>;

pub struct Compiler {
    config: CompilerConfig,
}

struct ClassArtifact {
    internal_name: String,
    bytes: Vec<u8>,
}

struct ClassPlan {
    unit: CompilationUnit,
    internal_name: String,
    source_file: String,
}

impl Compiler {
    pub fn new(config: CompilerConfig) -> Self {
        Self { config }
    }

    pub fn compile(self) -> CompileResult<()> {
        let catalog = build_class_catalog(&self.config.classpath, &self.config.source_files)?;
        let incremental = IncrementalBuild::from_config(&self.config)?;
        let mut errors = Vec::new();
        for source_file in &self.config.source_files {
            if let Err(error) = self.compile_file(source_file, &catalog, incremental.as_ref()) {
                errors.extend(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn compile_file(
        &self,
        source_file: &str,
        catalog: &ClassCatalog,
        incremental: Option<&IncrementalBuild>,
    ) -> CompileResult<()> {
        let source = read_source_file(source_file)?;
        let plan = plan_source(source_file, &source, catalog)?;
        let class_path = class_file_path(&self.config.output_dir, &plan.internal_name);

        if incremental.is_some_and(|incremental| incremental.class_is_fresh(&class_path)) {
            return Ok(());
        }

        let artifacts = compile_plan(source_file, &source, catalog, plan)?;
        for artifact in artifacts {
            write_class_file(&self.config.output_dir, &artifact)?;
        }
        Ok(())
    }
}

fn read_source_file(path: &str) -> CompileResult<String> {
    std::fs::read_to_string(path).map_err(|e| vec![format!("failed to read {}: {}", path, e)])
}

fn plan_source(filename: &str, source: &str, catalog: &ClassCatalog) -> CompileResult<ClassPlan> {
    let unit = parse_and_lower(filename, source, catalog)?;
    let internal_name = top_level_class_name(filename, &unit)?;
    let source_file = source_file_attribute_name(filename);

    Ok(ClassPlan {
        unit,
        internal_name,
        source_file,
    })
}

fn compile_plan(
    filename: &str,
    source: &str,
    catalog: &ClassCatalog,
    plan: ClassPlan,
) -> CompileResult<Vec<ClassArtifact>> {
    let classes = crate::bytecode::class_gen::gen_classes_with_source_file(
        &plan.unit,
        catalog,
        Some(&plan.source_file),
    )
    .map_err(|e| render_bytecode_error(filename, source, &e))?;

    Ok(classes
        .into_iter()
        .map(|class| ClassArtifact {
            internal_name: class.internal_name,
            bytes: class.bytes,
        })
        .collect())
}

fn source_file_attribute_name(filename: &str) -> String {
    Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(filename)
        .to_string()
}

fn parse_and_lower(
    filename: &str,
    source: &str,
    catalog: &ClassCatalog,
) -> CompileResult<CompilationUnit> {
    let parse = crate::parser::Parser::parse(source);
    if !parse.errors.is_empty() {
        let diagnostics = parse
            .errors
            .iter()
            .map(|error| error.diagnostic())
            .collect::<Vec<_>>();
        return Err(render_diagnostics(
            SourceFile::new(filename, source),
            &diagnostics,
        ));
    }

    let root = JavaSyntaxNode::new_root(parse.green_node);
    crate::hir::lowering::lower_with_catalog(&root, catalog)
        .map_err(|e| render_lower_error(filename, source, &e))
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
