use javac_ast::JavaSyntaxKind;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxErrorKind {
    ExpectedToken { expected: JavaSyntaxKind, found: JavaSyntaxKind },
    UnexpectedToken { found: JavaSyntaxKind },
    MissingSemicolon,
    MissingClosingBrace,
    MissingClosingParen,
    MissingClosingBracket,
    InvalidExpression,
    DuplicateModifier,
    InvalidModifier,
    UnclosedComment,
    UnclosedStringLiteral,
    InvalidNumericLiteral,
    UnclosedCharLiteral,
    ExtraTokens,
}

impl fmt::Display for SyntaxErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyntaxErrorKind::ExpectedToken { expected, found } => {
                write!(f, "expected {:?}, found {:?}", expected, found)
            }
            SyntaxErrorKind::UnexpectedToken { found } => {
                write!(f, "unexpected token {:?}", found)
            }
            SyntaxErrorKind::MissingSemicolon => write!(f, "missing semicolon"),
            SyntaxErrorKind::MissingClosingBrace => write!(f, "missing closing brace"),
            SyntaxErrorKind::MissingClosingParen => write!(f, "missing closing parenthesis"),
            SyntaxErrorKind::MissingClosingBracket => write!(f, "missing closing bracket"),
            SyntaxErrorKind::InvalidExpression => write!(f, "invalid expression"),
            SyntaxErrorKind::DuplicateModifier => write!(f, "duplicate modifier"),
            SyntaxErrorKind::InvalidModifier => write!(f, "invalid modifier"),
            SyntaxErrorKind::UnclosedComment => write!(f, "unclosed comment"),
            SyntaxErrorKind::UnclosedStringLiteral => write!(f, "unclosed string literal"),
            SyntaxErrorKind::InvalidNumericLiteral => write!(f, "invalid numeric literal"),
            SyntaxErrorKind::UnclosedCharLiteral => write!(f, "unclosed character literal"),
            SyntaxErrorKind::ExtraTokens => write!(f, "extra tokens after end of input"),
        }
    }
}