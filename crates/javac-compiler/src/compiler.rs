use crate::classpath::build_class_catalog;
use crate::config::CompilerConfig;
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

impl Compiler {
    pub fn new(config: CompilerConfig) -> Self {
        Self { config }
    }

    pub fn compile(self) -> CompileResult<()> {
        let catalog = build_class_catalog(&self.config.classpath, &self.config.source_files)?;
        let mut errors = Vec::new();
        for source_file in &self.config.source_files {
            if let Err(error) = self.compile_file(source_file, &catalog) {
                errors.extend(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn compile_file(&self, source_file: &str, catalog: &ClassCatalog) -> CompileResult<()> {
        let source = read_source_file(source_file)?;
        let artifact = compile_source(source_file, &source, catalog)?;
        write_class_file(&self.config.output_dir, &artifact)?;
        Ok(())
    }
}

fn read_source_file(path: &str) -> CompileResult<String> {
    std::fs::read_to_string(path).map_err(|e| vec![format!("failed to read {}: {}", path, e)])
}

fn compile_source(
    filename: &str,
    source: &str,
    catalog: &ClassCatalog,
) -> CompileResult<ClassArtifact> {
    let unit = parse_and_lower(filename, source, catalog)?;
    let internal_name = top_level_class_name(filename, &unit)?;
    let bytes = javac_bytecode::class_gen::gen_class(&unit)
        .map_err(|e| render_bytecode_error(filename, source, &e))?;

    Ok(ClassArtifact {
        internal_name,
        bytes,
    })
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
        LowerError::UnknownImport { name, line } | LowerError::UnknownType { name, line } => {
            line_range(source, *line as usize, Some(name.as_str()))
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
        .with_code("B0001")
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
    if let Some(needle) = needle {
        if let Some(relative_start) = source[line_start..line_end].find(needle) {
            let start = line_start + relative_start;
            return byte_range(start, start + needle.len());
        }
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
        LowerError::UnsupportedExpression => "unsupported expression here",
        LowerError::PatternVariableOutOfScope(_) => "pattern variable is not in scope",
        LowerError::MissingClassName => "class name is missing",
        LowerError::MissingMethodName => "name is missing",
        LowerError::MissingType => "type is missing",
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
        LowerError::UnsupportedExpression => {
            "simplify the expression or add compiler support for it"
        }
        LowerError::PatternVariableOutOfScope(_) => {
            "move the pattern variable use into the guarded branch"
        }
        LowerError::MissingClassName => "add an identifier after the class keyword",
        LowerError::MissingMethodName => "add the missing identifier",
        LowerError::MissingType => "add a valid Java type",
        LowerError::MissingImportName => "add a qualified import name",
        LowerError::UnknownImport { .. } => {
            "remove the import or add it to the supported class set"
        }
        LowerError::UnknownType { .. } => "import a supported class or use a known java.lang type",
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
