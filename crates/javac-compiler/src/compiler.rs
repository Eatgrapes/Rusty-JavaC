use crate::classpath::build_class_catalog;
use crate::config::CompilerConfig;
use crate::incremental::IncrementalBuild;
use javac_ast::JavaSyntaxNode;
use javac_bytecode::BytecodeError;
use javac_call_resolver::ClassCatalog;
use javac_diagnostics::{Diagnostic, SourceFile, render_diagnostics};
use javac_hir::hir::CompilationUnit;
use javac_hir::lowering::LowerError;
use std::path::{Path, PathBuf};
use text_size::{TextRange, TextSize};

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
    let classes = javac_bytecode::class_gen::gen_classes_with_source_file(
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
    let parse = javac_parser::Parser::parse(source);
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
    javac_hir::lowering::lower_with_catalog(&root, catalog)
        .map_err(|e| render_lower_error(filename, source, &e))
}

fn render_lower_error(filename: &str, source: &str, error: &LowerError) -> Vec<String> {
    let diagnostic = Diagnostic::error(error.to_string(), lower_error_range(source, error))
        .with_code("L0001")
        .with_primary_label(lower_error_label(error))
        .with_help(lower_error_help(error));

    render_diagnostics(SourceFile::new(filename, source), &[diagnostic])
}

fn lower_error_range(source: &str, error: &LowerError) -> TextRange {
    match error {
        LowerError::UnknownImport {
            name,
            line,
            range: Some(range),
        } => validated_range(source, *range)
            .unwrap_or_else(|| line_range(source, *line as usize, Some(name.as_str()))),
        LowerError::UnknownImport {
            name,
            line,
            range: None,
        }
        | LowerError::UnknownType { name, line } => {
            line_range(source, *line as usize, Some(name.as_str()))
        }
        LowerError::VarRequiresInitializer { line } => line_range(source, *line as usize, None),
        LowerError::UnsupportedExpressionAt {
            line,
            range: Some(range),
        } => validated_range(source, *range)
            .unwrap_or_else(|| line_range(source, *line as usize, None)),
        LowerError::UnsupportedExpressionAt { line, range: None } => {
            line_range(source, *line as usize, None)
        }
        _ => source_start_range(source),
    }
}

fn render_bytecode_error(filename: &str, source: &str, error: &BytecodeError) -> Vec<String> {
    let Some(line) = error.line else {
        return vec![format!("{}: {}", filename, error)];
    };

    let range = line_range(source, line as usize, error.needle.as_deref());
    let mut diagnostic = Diagnostic::error(error.message.clone(), range)
        .with_code(error.code)
        .with_primary_label(
            error
                .label
                .clone()
                .unwrap_or_else(|| "failed to compile this expression".to_string()),
        );

    if let Some(help) = &error.help {
        diagnostic = diagnostic.with_help(help.as_str());
    }

    render_diagnostics(SourceFile::new(filename, source), &[diagnostic])
}

fn source_start_range(source: &str) -> TextRange {
    let start = source
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end = source[start..]
        .chars()
        .next()
        .map(|ch| start + ch.len_utf8())
        .unwrap_or(start + 1);
    byte_range(start, end)
}

fn line_range(source: &str, line: usize, needle: Option<&str>) -> TextRange {
    let (line_start, line_end) = line_byte_bounds(source, line);
    if let Some(needle) = needle
        && let Some(relative_start) = source[line_start..line_end].find(needle)
    {
        let start = line_start + relative_start;
        return byte_range(start, start + needle.len());
    }

    let start = line_start;
    let end = line_end.max(start + 1);
    byte_range(start, end)
}

fn byte_range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::from(start.min(u32::MAX as usize) as u32),
        TextSize::from(end.min(u32::MAX as usize) as u32),
    )
}

fn validated_range(source: &str, range: TextRange) -> Option<TextRange> {
    let start = u32::from(range.start()) as usize;
    let end = u32::from(range.end()) as usize;
    (start < end && end <= source.len()).then_some(range)
}

fn line_byte_bounds(source: &str, target_line: usize) -> (usize, usize) {
    let mut current_line = 1;
    let mut line_start = 0;

    for (index, ch) in source.char_indices() {
        if current_line == target_line {
            let line_end = source[index..]
                .find('\n')
                .map(|offset| index + offset)
                .unwrap_or(source.len());
            return (line_start, line_end);
        }

        if ch == '\n' {
            current_line += 1;
            line_start = index + 1;
        }
    }

    if current_line == target_line {
        (line_start, source.len())
    } else {
        (source.len(), source.len())
    }
}

fn lower_error_label(error: &LowerError) -> &'static str {
    match error {
        LowerError::ExpectedSingleTopLevelClass => "missing class declaration",
        LowerError::UnsupportedExpression | LowerError::UnsupportedExpressionAt { .. } => {
            "unsupported expression here"
        }
        LowerError::PatternVariableOutOfScope(_) => "pattern variable is not in scope",
        LowerError::MissingClassName => "class name is missing",
        LowerError::MissingMethodName => "name is missing",
        LowerError::MissingType => "type is missing",
        LowerError::VarRequiresInitializer { .. } => "initializer is missing",
        LowerError::MissingImportName => "import name is missing",
        LowerError::UnknownImport { .. } => "unresolved import",
        LowerError::UnknownType { .. } => "unresolved type",
        LowerError::UnsupportedTypeDeclaration => "unsupported declaration",
        LowerError::UnsupportedClassMember => "unsupported member",
        LowerError::ExpectedCompilationUnit => "expected Java source",
    }
}

fn lower_error_help(error: &LowerError) -> &'static str {
    match error {
        LowerError::ExpectedSingleTopLevelClass => "add one top-level class declaration",
        LowerError::UnsupportedExpression | LowerError::UnsupportedExpressionAt { .. } => {
            "simplify the expression or add compiler support for it"
        }
        LowerError::PatternVariableOutOfScope(_) => {
            "move the pattern variable use into the guarded branch"
        }
        LowerError::MissingClassName => "add an identifier after the class keyword",
        LowerError::MissingMethodName => "add the missing identifier",
        LowerError::MissingType => "add a valid Java type",
        LowerError::VarRequiresInitializer { .. } => {
            "add an initializer or write the explicit type"
        }
        LowerError::MissingImportName => "add a qualified import name",
        LowerError::UnknownImport { .. } => {
            "check the import spelling or add the class, jar, or source directory with --class-path"
        }
        LowerError::UnknownType { .. } => {
            "import the type, use a java.lang type, or add it with --class-path"
        }
        LowerError::UnsupportedTypeDeclaration => "use a class declaration",
        LowerError::UnsupportedClassMember => "remove or simplify this class member",
        LowerError::ExpectedCompilationUnit => "provide a Java compilation unit",
    }
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
