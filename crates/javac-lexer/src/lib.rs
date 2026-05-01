mod token;
#[allow(dead_code)]
mod unicode_esc;

use javac_ast::JavaSyntaxKind;
use text_size::{TextRange, TextSize};
use token::TextualToken;

pub use token::TextualToken as RawToken;

pub struct Lexer<'src> {
    inner: logos::Lexer<'src, TextualToken>,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { inner: logos::Lexer::new(source) }
    }
}

fn raw_to_syntax(raw: TextualToken) -> JavaSyntaxKind {
    use JavaSyntaxKind::*;
    match raw {
        TextualToken::Abstract => AbstractKw,
        TextualToken::Assert => AssertKw,
        TextualToken::Boolean => BooleanKw,
        TextualToken::Break => BreakKw,
        TextualToken::Byte => ByteKw,
        TextualToken::Case => CaseKw,
        TextualToken::Catch => CatchKw,
        TextualToken::Char => CharKw,
        TextualToken::Class => ClassKw,
        TextualToken::Continue => ContinueKw,
        TextualToken::Default => DefaultKw,
        TextualToken::Do => DoKw,
        TextualToken::Double => DoubleKw,
        TextualToken::Else => ElseKw,
        TextualToken::Enum => EnumKw,
        TextualToken::Extends => ExtendsKw,
        TextualToken::Final => FinalKw,
        TextualToken::Finally => FinallyKw,
        TextualToken::Float => FloatKw,
        TextualToken::For => ForKw,
        TextualToken::If => IfKw,
        TextualToken::Implements => ImplementsKw,
        TextualToken::Import => ImportKw,
        TextualToken::Instanceof => InstanceofKw,
        TextualToken::Int => IntKw,
        TextualToken::Interface => InterfaceKw,
        TextualToken::Long => LongKw,
        TextualToken::Native => NativeKw,
        TextualToken::New => NewKw,
        TextualToken::Package => PackageKw,
        TextualToken::Private => PrivateKw,
        TextualToken::Protected => ProtectedKw,
        TextualToken::Public => PublicKw,
        TextualToken::Return => ReturnKw,
        TextualToken::Short => ShortKw,
        TextualToken::Static => StaticKw,
        TextualToken::Strictfp => StrictfpKw,
        TextualToken::Super => SuperKw,
        TextualToken::Switch => SwitchKw,
        TextualToken::Synchronized => SynchronizedKw,
        TextualToken::This => ThisKw,
        TextualToken::Throw => ThrowKw,
        TextualToken::Throws => ThrowsKw,
        TextualToken::Transient => TransientKw,
        TextualToken::Try => TryKw,
        TextualToken::Void => VoidKw,
        TextualToken::Volatile => VolatileKw,
        TextualToken::While => WhileKw,
        TextualToken::Yield => YieldKw,
        TextualToken::Record => RecordKw,
        TextualToken::Sealed => SealedKw,
        TextualToken::NonSealed => NonSealedKw,
        TextualToken::Permits => PermitsKw,
        TextualToken::Var => VarKw,

        TextualToken::HexLiteral => IntLiteral,
        TextualToken::BinLiteral => IntLiteral,
        TextualToken::LongLiteral => LongLiteral,
        TextualToken::IntLiteral => IntLiteral,
        TextualToken::FloatLiteral => FloatLiteral,
        TextualToken::FloatLiteralExp => FloatLiteral,
        TextualToken::FloatLiteralDot => FloatLiteral,
        TextualToken::FloatLiteralSuffix => FloatLiteral,
        TextualToken::CharLiteral => CharLiteral,
        TextualToken::StringLiteral => StringLiteral,
        TextualToken::TextBlock => TextBlockLiteral,
        TextualToken::True => TrueKw,
        TextualToken::False => FalseKw,
        TextualToken::Null => NullKw,

        TextualToken::Ident => Ident,

        TextualToken::LBrace => LBrace,
        TextualToken::RBrace => RBrace,
        TextualToken::LBrack => LBrack,
        TextualToken::RBrack => RBrack,
        TextualToken::LParen => LParen,
        TextualToken::RParen => RParen,
        TextualToken::Semi => Semi,
        TextualToken::Comma => Comma,
        TextualToken::Dot => Dot,
        TextualToken::Ellipsis => Ellipsis,
        TextualToken::At => At,
        TextualToken::ColonColon => ColonColon,
        TextualToken::Arrow => Arrow,

        TextualToken::Eq => Eq,
        TextualToken::Gt => Gt,
        TextualToken::Lt => Lt,
        TextualToken::Bang => Bang,
        TextualToken::Tilde => Tilde,
        TextualToken::Question => Question,
        TextualToken::Colon => Colon,

        TextualToken::EqEq => EqEq,
        TextualToken::Le => Le,
        TextualToken::Ge => Ge,
        TextualToken::Neq => Neq,

        TextualToken::Inc => Inc,
        TextualToken::Dec => Dec,

        TextualToken::AmpAmp => AmpAmp,
        TextualToken::PipePipe => PipePipe,

        TextualToken::Plus => Plus,
        TextualToken::Minus => Minus,
        TextualToken::Star => Star,
        TextualToken::Slash => Slash,
        TextualToken::Amp => Amp,
        TextualToken::Pipe => Pipe,
        TextualToken::Caret => Caret,
        TextualToken::Percent => Percent,

        TextualToken::LtLt => LtLt,
        TextualToken::GtGt => GtGt,
        TextualToken::GtGtGt => GtGtGt,

        TextualToken::PlusEq => PlusEq,
        TextualToken::MinusEq => MinusEq,
        TextualToken::StarEq => StarEq,
        TextualToken::SlashEq => SlashEq,
        TextualToken::AmpEq => AmpEq,
        TextualToken::PipeEq => PipeEq,
        TextualToken::CaretEq => CaretEq,
        TextualToken::PercentEq => PercentEq,
        TextualToken::LtLtEq => LtLtEq,
        TextualToken::GtGtEq => GtGtEq,
        TextualToken::GtGtGtEq => GtGtGtEq,

        TextualToken::Underscore => Underscore,
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