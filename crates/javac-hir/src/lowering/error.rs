use std::fmt;

pub type LowerResult<T> = Result<T, LowerError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    ExpectedCompilationUnit,
    PackagesNotSupported,
    UnsupportedTypeDeclaration,
    ExpectedSingleTopLevelClass,
    UnsupportedClassMember,
    MissingClassName,
    MissingImportName,
    MissingMethodName,
    MissingType,
    UnsupportedExpression,
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            LowerError::ExpectedCompilationUnit => "expected compilation unit",
            LowerError::PackagesNotSupported => "packages are not supported yet",
            LowerError::UnsupportedTypeDeclaration => "only class declarations are supported yet",
            LowerError::ExpectedSingleTopLevelClass => "expected one top-level class",
            LowerError::UnsupportedClassMember => "unsupported class member",
            LowerError::MissingClassName => "class declaration is missing a name",
            LowerError::MissingImportName => "import declaration is missing a name",
            LowerError::MissingMethodName => "method declaration is missing a name",
            LowerError::MissingType => "type syntax is missing",
            LowerError::UnsupportedExpression => "unsupported expression",
        };
        f.write_str(message)
    }
}
