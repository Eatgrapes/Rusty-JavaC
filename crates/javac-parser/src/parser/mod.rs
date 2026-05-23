mod expr;
mod member;
mod stmt;
mod top_level;
mod ty;
mod type_decl;

pub(crate) use javac_ast::JavaSyntaxKind;
use javac_lexer::Lexer;
use rowan::GreenNodeBuilder;

pub struct Parse {
    pub green_node: rowan::GreenNode,
    pub errors: Vec<ParseError>,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
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
            self.err(format!("expected {:?}, got {:?}", k, self.kind()));
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
        let off = self.tokens.get(self.pos).map(|t| t.offset).unwrap_or(0);
        self.errors.push(ParseError {
            message: msg.into(),
            offset: off,
        });
    }

    pub(crate) fn err_and_bump(&mut self, msg: impl Into<String>) {
        self.err(msg);
        self.bump();
    }
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
