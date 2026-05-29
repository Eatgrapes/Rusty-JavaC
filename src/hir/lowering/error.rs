use text_size::TextRange;

pub type LowerResult<T> = Result<T, LowerError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LowerError {
    #[error("expected compilation unit")]
    ExpectedCompilationUnit,
    #[error("only class declarations are supported yet")]
    UnsupportedTypeDeclaration,
    #[error("expected one top-level class")]
    ExpectedSingleTopLevelClass,
    #[error("unsupported class member")]
    UnsupportedClassMember,
    #[error("class declaration is missing a name")]
    MissingClassName,
    #[error("import declaration is missing a name")]
    MissingImportName,
    #[error("cannot find symbol: import `{name}`")]
    UnknownImport {
        name: String,
        line: u16,
        range: Option<TextRange>,
    },
    #[error("method declaration is missing a name")]
    MissingMethodName,
    #[error("type syntax is missing")]
    MissingType,
    #[error("local variable type inference requires an initializer")]
    VarRequiresInitializer { line: u16 },
    #[error("cannot find symbol: class `{name}`")]
    UnknownType { name: String, line: u16 },
    #[error("unsupported expression")]
    UnsupportedExpression,
    #[error("unsupported expression")]
    UnsupportedExpressionAt { line: u16, range: Option<TextRange> },
    #[error("pattern variable `{0}` is not in scope")]
    PatternVariableOutOfScope(String),
}
