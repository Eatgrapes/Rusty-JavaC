mod token;
#[allow(dead_code)]
mod unicode_esc;

use javac_ast::JavaSyntaxKind;
use text_size::{TextRange, TextSize};
use token::TextualToken;
pub use token::raw_to_syntax;

pub use token::TextualToken as RawToken;

pub struct Lexer<'src> {
    inner: logos::Lexer<'src, TextualToken>,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { inner: logos::Lexer::new(source) }
    }
}

pub struct LexedToken {
    pub kind: JavaSyntaxKind,
    pub range: TextRange,
    pub text: String,
}

impl<'src> Iterator for Lexer<'src> {
    type Item = LexedToken;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.inner.next()?;
        let span = self.inner.span();
        let kind = match token {
            Ok(t) => raw_to_syntax(t),
            Err(()) => JavaSyntaxKind::Error,
        };
        let start = TextSize::new(span.start as u32);
        let end = TextSize::new(span.end as u32);
        Some(LexedToken {
            kind,
            range: TextRange::new(start, end),
            text: self.inner.slice().to_string(),
        })
    }
}