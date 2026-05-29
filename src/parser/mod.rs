mod expr;
mod member;
mod stmt;
mod top_level;
mod ty;
mod type_decl;

pub(crate) use crate::ast::JavaSyntaxKind;
use crate::diagnostics::Diagnostic;
use crate::lexer::Lexer;
use rowan::GreenNodeBuilder;
use text_size::{TextRange, TextSize};

pub struct Parse {
    pub green_node: rowan::GreenNode,
    pub errors: Vec<ParseError>,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
    pub len: usize,
    pub label: String,
    pub help: Option<String>,
}

impl ParseError {
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.message.clone(), self.range())
            .with_code("P0001")
            .with_primary_label(self.label.clone())
            .with_help(
                self.help
                    .clone()
                    .unwrap_or_else(|| "check the token at the highlighted position".to_string()),
            )
    }

    fn range(&self) -> TextRange {
        let start = TextSize::from(self.offset.min(u32::MAX as usize) as u32);
        let end = TextSize::from((self.offset + self.len).min(u32::MAX as usize) as u32);
        TextRange::new(start, end)
    }
}

pub(crate) struct Token {
    pub(crate) kind: JavaSyntaxKind,
    pub(crate) text: String,
    pub(crate) offset: usize,
}

pub struct Parser {
    pub(crate) source: String,
    pub(crate) tokens: Vec<Token>,
    pub(crate) pos: usize,
    trivia_end: usize,
    pub(crate) builder: GreenNodeBuilder<'static>,
    pub(crate) errors: Vec<ParseError>,
}

impl Parser {
    pub fn parse(source: &str) -> Parse {
        let lexer = Lexer::new(source);
        let tokens: Vec<_> = lexer
            .map(|t| Token {
                kind: t.kind,
                text: t.text,
                offset: u32::from(t.range.start()) as usize,
            })
            .collect();

        let mut parser = Parser {
            source: source.to_string(),
            tokens,
            pos: 0,
            trivia_end: 0,
            builder: GreenNodeBuilder::new(),
            errors: Vec::new(),
        };

        top_level::compilation_unit(&mut parser);
        let green_node = parser.builder.finish();

        Parse {
            green_node,
            errors: parser.errors,
        }
    }

    pub(crate) fn start(&mut self) -> Marker {
        let _pos = self.pos;
        let checkpoint = self.builder.checkpoint();
        Marker { _pos, checkpoint }
    }

    pub(crate) fn kind(&self) -> JavaSyntaxKind {
        self.tokens
            .get(self.pos)
            .map(|t| t.kind)
            .unwrap_or(JavaSyntaxKind::Error)
    }

    pub(crate) fn look(&self, ahead: usize) -> JavaSyntaxKind {
        self.tokens
            .get(self.pos + ahead)
            .map(|t| t.kind)
            .unwrap_or(JavaSyntaxKind::Error)
    }

    pub(crate) fn at(&self, k: JavaSyntaxKind) -> bool {
        self.kind() == k
    }

    pub(crate) fn at_any(&self, ks: &[JavaSyntaxKind]) -> bool {
        ks.contains(&self.kind())
    }

    pub(crate) fn bump(&mut self) {
        if self.pos < self.tokens.len() {
            let tok = &self.tokens[self.pos];
            if self.trivia_end < tok.offset {
                self.builder.token(
                    JavaSyntaxKind::Whitespace.into(),
                    &self.source[self.trivia_end..tok.offset],
                );
            }
            self.builder.token(tok.kind.into(), tok.text.as_str());
            self.trivia_end = tok.offset + tok.text.len();
            self.pos += 1;
        }
    }

    pub(crate) fn expect(&mut self, k: JavaSyntaxKind) {
        if self.at(k) {
            self.bump();
        } else {
            self.err_expected(k);
        }
    }

    pub(crate) fn eat(&mut self, k: JavaSyntaxKind) -> bool {
        if self.at(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn err(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        let (offset, len) = self.current_span();
        let found = token_display(self.kind());
        self.errors.push(ParseError {
            message: msg,
            offset,
            len,
            label: format!("found {found}"),
            help: None,
        });
    }

    pub(crate) fn err_and_bump(&mut self, msg: impl Into<String>) {
        self.err(msg);
        self.bump();
    }

    fn err_expected(&mut self, expected: JavaSyntaxKind) {
        let (offset, len) = self.current_span();
        let expected = token_display(expected);
        let found = token_display(self.kind());
        self.errors.push(ParseError {
            message: format!("expected {expected}, found {found}"),
            offset,
            len,
            label: format!("expected {expected} here"),
            help: Some(format!("insert {expected} or remove {found}")),
        });
    }

    fn current_span(&self) -> (usize, usize) {
        self.tokens
            .get(self.pos)
            .map(|token| (token.offset, token.text.len().max(1)))
            .unwrap_or_else(|| (self.source.len(), 0))
    }
}

fn token_display(kind: JavaSyntaxKind) -> String {
    use JavaSyntaxKind::*;
    match kind {
        AbstractKw => "`abstract`",
        AssertKw => "`assert`",
        BooleanKw => "`boolean`",
        BreakKw => "`break`",
        ByteKw => "`byte`",
        CaseKw => "`case`",
        CatchKw => "`catch`",
        CharKw => "`char`",
        ClassKw => "`class`",
        ContinueKw => "`continue`",
        DefaultKw => "`default`",
        DoKw => "`do`",
        DoubleKw => "`double`",
        ElseKw => "`else`",
        EnumKw => "`enum`",
        ExtendsKw => "`extends`",
        FinalKw => "`final`",
        FinallyKw => "`finally`",
        FloatKw => "`float`",
        ForKw => "`for`",
        IfKw => "`if`",
        ImplementsKw => "`implements`",
        ImportKw => "`import`",
        InstanceofKw => "`instanceof`",
        IntKw => "`int`",
        InterfaceKw => "`interface`",
        LongKw => "`long`",
        NativeKw => "`native`",
        NewKw => "`new`",
        PackageKw => "`package`",
        PrivateKw => "`private`",
        ProtectedKw => "`protected`",
        PublicKw => "`public`",
        ReturnKw => "`return`",
        ShortKw => "`short`",
        StaticKw => "`static`",
        StrictfpKw => "`strictfp`",
        SuperKw => "`super`",
        SwitchKw => "`switch`",
        SynchronizedKw => "`synchronized`",
        ThisKw => "`this`",
        ThrowKw => "`throw`",
        ThrowsKw => "`throws`",
        TransientKw => "`transient`",
        TryKw => "`try`",
        VoidKw => "`void`",
        VolatileKw => "`volatile`",
        WhileKw => "`while`",
        YieldKw => "`yield`",
        RecordKw => "`record`",
        SealedKw => "`sealed`",
        NonSealedKw => "`non-sealed`",
        PermitsKw => "`permits`",
        VarKw => "`var`",
        IntLiteral => "integer literal",
        LongLiteral => "long literal",
        FloatLiteral => "float literal",
        DoubleLiteral => "double literal",
        CharLiteral => "character literal",
        StringLiteral => "string literal",
        TextBlockLiteral => "text block literal",
        TrueKw => "`true`",
        FalseKw => "`false`",
        NullKw => "`null`",
        Ident => "identifier",
        LBrace => "`{`",
        RBrace => "`}`",
        LBrack => "`[`",
        RBrack => "`]`",
        LParen => "`(`",
        RParen => "`)`",
        Semi => "`;`",
        Comma => "`,`",
        Dot => "`.`",
        Ellipsis => "`...`",
        At => "`@`",
        ColonColon => "`::`",
        Arrow => "`->`",
        Eq => "`=`",
        Gt => "`>`",
        Lt => "`<`",
        Bang => "`!`",
        Tilde => "`~`",
        Question => "`?`",
        Colon => "`:`",
        EqEq => "`==`",
        Le => "`<=`",
        Ge => "`>=`",
        Neq => "`!=`",
        Inc => "`++`",
        Dec => "`--`",
        AmpAmp => "`&&`",
        PipePipe => "`||`",
        Plus => "`+`",
        Minus => "`-`",
        Star => "`*`",
        Slash => "`/`",
        Amp => "`&`",
        Pipe => "`|`",
        Caret => "`^`",
        Percent => "`%`",
        LtLt => "`<<`",
        GtGt => "`>>`",
        GtGtGt => "`>>>`",
        PlusEq => "`+=`",
        MinusEq => "`-=`",
        StarEq => "`*=`",
        SlashEq => "`/=`",
        AmpEq => "`&=`",
        PipeEq => "`|=`",
        CaretEq => "`^=`",
        PercentEq => "`%=`",
        LtLtEq => "`<<=`",
        GtGtEq => "`>>=`",
        GtGtGtEq => "`>>>=`",
        Underscore => "`_`",
        Error => "end of input",
        _ => "syntax",
    }
    .to_string()
}

pub(crate) struct Marker {
    _pos: usize,
    checkpoint: rowan::Checkpoint,
}

impl Marker {
    pub(crate) fn complete(self, p: &mut Parser, kind: JavaSyntaxKind) {
        p.builder.start_node_at(self.checkpoint, kind.into());
        p.builder.finish_node();
    }

    pub(crate) fn abandon(self, _p: &mut Parser) {}
}

pub(crate) struct Lookahead<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Lookahead<'a> {
    pub(crate) fn at(&self, kind: JavaSyntaxKind) -> bool {
        self.kind() == kind
    }

    pub(crate) fn kind(&self) -> JavaSyntaxKind {
        self.tokens
            .get(self.pos)
            .map(|t| t.kind)
            .unwrap_or(JavaSyntaxKind::Error)
    }

    pub(crate) fn at_any(&self, ks: &[JavaSyntaxKind]) -> bool {
        ks.contains(&self.kind())
    }

    pub(crate) fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    pub(crate) fn eat(&mut self, kind: JavaSyntaxKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn skip_balanced(&mut self, open: JavaSyntaxKind, close: JavaSyntaxKind) {
        if !self.eat(open) {
            return;
        }
        let mut depth = 1usize;
        while depth > 0 && self.pos < self.tokens.len() {
            if self.at(open) {
                depth += 1;
            } else if self.at(close) {
                depth -= 1;
            }
            self.advance();
        }
    }

    pub(crate) fn skip_annotations(&mut self) {
        use JavaSyntaxKind::*;
        while self.eat(At) {
            self.eat(Ident);
            self.skip_balanced(LParen, RParen);
        }
    }

    pub(crate) fn skip_type(&mut self) {
        use JavaSyntaxKind::*;
        let primitives = [
            IntKw, LongKw, ShortKw, ByteKw, CharKw, FloatKw, DoubleKw, BooleanKw, VoidKw,
        ];
        if self.at_any(&primitives) {
            self.advance();
        } else {
            while self.eat(Ident) {
                self.skip_balanced(Lt, Gt);
                if !self.eat(Dot) {
                    break;
                }
            }
        }
    }

    pub(crate) fn skip_array_dims(&mut self) {
        use JavaSyntaxKind::*;
        while self.at(LBrack)
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.kind == RBrack)
        {
            self.pos += 2;
        }
    }
}

impl Parser {
    pub(crate) fn lookahead(&self) -> Lookahead<'_> {
        Lookahead {
            tokens: &self.tokens,
            pos: self.pos,
        }
    }
}
