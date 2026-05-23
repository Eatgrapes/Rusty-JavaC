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
    #[error("method declaration is missing a name")]
    MissingMethodName,
    #[error("type syntax is missing")]
    MissingType,
    #[error("unsupported expression")]
    UnsupportedExpression,
    #[error("pattern variable `{0}` is not in scope")]
    PatternVariableOutOfScope(String),
}
