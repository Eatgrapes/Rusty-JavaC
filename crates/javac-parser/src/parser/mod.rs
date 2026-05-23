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
