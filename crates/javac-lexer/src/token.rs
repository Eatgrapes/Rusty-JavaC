use logos::Logos;
use javac_ast::JavaSyntaxKind;

macro_rules! define_tokens {
    (
        $(
            $(#[$attr:meta])*
            $variant:ident => $kind:ident,
        )*
    ) => {
        #[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
        #[logos(skip r"[ \t\r\n\f]+")]
        #[logos(skip(r"//[^\n]*", allow_greedy = true))]
        #[logos(skip(r"/\*[\s\S]*?\*/", allow_greedy = true))]
        pub enum TextualToken {
            $(
                $(#[$attr])*
                $variant,
            )*
        }

        pub fn raw_to_syntax(raw: TextualToken) -> JavaSyntaxKind {
            match raw {
                $(
                    TextualToken::$variant => JavaSyntaxKind::$kind,
                )*
            }
        }
    };
}

define_tokens! {
    #[token("abstract")] Abstract => AbstractKw,
    #[token("assert")] Assert => AssertKw,
    #[token("boolean")] Boolean => BooleanKw,
    #[token("break")] Break => BreakKw,
    #[token("byte")] Byte => ByteKw,
    #[token("case")] Case => CaseKw,
    #[token("catch")] Catch => CatchKw,
    #[token("char")] Char => CharKw,
    #[token("class")] Class => ClassKw,
    #[token("continue")] Continue => ContinueKw,
    #[token("default")] Default => DefaultKw,
    #[token("do")] Do => DoKw,
    #[token("double")] Double => DoubleKw,
    #[token("else")] Else => ElseKw,
    #[token("enum")] Enum => EnumKw,
    #[token("extends")] Extends => ExtendsKw,
    #[token("final")] Final => FinalKw,
    #[token("finally")] Finally => FinallyKw,
    #[token("float")] Float => FloatKw,
    #[token("for")] For => ForKw,
    #[token("if")] If => IfKw,
    #[token("implements")] Implements => ImplementsKw,
    #[token("import")] Import => ImportKw,
    #[token("instanceof")] Instanceof => InstanceofKw,
    #[token("int")] Int => IntKw,
    #[token("interface")] Interface => InterfaceKw,
    #[token("long")] Long => LongKw,
    #[token("native")] Native => NativeKw,
    #[token("new")] New => NewKw,
    #[token("package")] Package => PackageKw,
    #[token("private")] Private => PrivateKw,
    #[token("protected")] Protected => ProtectedKw,
    #[token("public")] Public => PublicKw,
    #[token("return")] Return => ReturnKw,
    #[token("short")] Short => ShortKw,
    #[token("static")] Static => StaticKw,
    #[token("strictfp")] Strictfp => StrictfpKw,
    #[token("super")] Super => SuperKw,
    #[token("switch")] Switch => SwitchKw,
    #[token("synchronized")] Synchronized => SynchronizedKw,
    #[token("this")] This => ThisKw,
    #[token("throw")] Throw => ThrowKw,
    #[token("throws")] Throws => ThrowsKw,
    #[token("transient")] Transient => TransientKw,
    #[token("try")] Try => TryKw,
    #[token("void")] Void => VoidKw,
    #[token("volatile")] Volatile => VolatileKw,
    #[token("while")] While => WhileKw,
    #[token("yield")] Yield => YieldKw,
    #[token("record")] Record => RecordKw,
    #[token("sealed")] Sealed => SealedKw,
    #[token("non-sealed")] NonSealed => NonSealedKw,
    #[token("permits")] Permits => PermitsKw,
    #[token("var")] Var => VarKw,

    #[regex(r"0[xX][0-9a-fA-F_]+[lL]?", priority = 3)] HexLiteral => IntLiteral,
    #[regex(r"0[bB][01_]+[lL]?", priority = 3)] BinLiteral => IntLiteral,
    #[regex(r"[0-9][0-9_]*[lL]", priority = 4)] LongLiteral => LongLiteral,
    #[regex(r"[0-9][0-9_]*", priority = 2)] IntLiteral => IntLiteral,
    #[regex(r"[0-9][0-9_]*\.[0-9_]*([eE][+-]?[0-9_]+)?[fFdD]?", priority = 5)] FloatLiteral => FloatLiteral,
    #[regex(r"[0-9][0-9_]*[eE][+-]?[0-9_]+[fFdD]?", priority = 5)] FloatLiteralExp => FloatLiteral,
    #[regex(r"\.[0-9][0-9_]*([eE][+-]?[0-9_]+)?[fFdD]?", priority = 6)] FloatLiteralDot => FloatLiteral,
    #[regex(r"[0-9][0-9_]*[fFdD]", priority = 4)] FloatLiteralSuffix => FloatLiteral,
    #[regex(r"'([^'\\\r\n]|\\.)*'")] CharLiteral => CharLiteral,
    #[regex(r#""([^"\\\r\n]|\\.)*""#)] StringLiteral => StringLiteral,
    #[regex(r#""""[ \t]*\r?\n[\s\S]*?"""""#)] TextBlock => TextBlockLiteral,

    #[token("true")] True => TrueKw,
    #[token("false")] False => FalseKw,
    #[token("null")] Null => NullKw,

    #[regex(r"[a-zA-Z_$][a-zA-Z0-9_$]*", priority = 1)] Ident => Ident,

    #[token("{")] LBrace => LBrace,
    #[token("}")] RBrace => RBrace,
    #[token("[")] LBrack => LBrack,
    #[token("]")] RBrack => RBrack,
    #[token("(")] LParen => LParen,
    #[token(")")] RParen => RParen,
    #[token(";")] Semi => Semi,
    #[token(",")] Comma => Comma,
    #[token(".")] Dot => Dot,
    #[token("...")] Ellipsis => Ellipsis,
    #[token("@")] At => At,
    #[token("::")] ColonColon => ColonColon,
    #[token("->")] Arrow => Arrow,

    #[token("=")] Eq => Eq,
    #[token(">")] Gt => Gt,
    #[token("<")] Lt => Lt,
    #[token("!")] Bang => Bang,
    #[token("~")] Tilde => Tilde,
    #[token("?")] Question => Question,
    #[token(":")] Colon => Colon,
    #[token("==")] EqEq => EqEq,
    #[token("<=")] Le => Le,
    #[token(">=")] Ge => Ge,
    #[token("!=")] Neq => Neq,
    #[token("++")] Inc => Inc,
    #[token("--")] Dec => Dec,
    #[token("&&")] AmpAmp => AmpAmp,
    #[token("||")] PipePipe => PipePipe,
    #[token("+")] Plus => Plus,
    #[token("-")] Minus => Minus,
    #[token("*")] Star => Star,
    #[token("/")] Slash => Slash,
    #[token("&")] Amp => Amp,
    #[token("|")] Pipe => Pipe,
    #[token("^")] Caret => Caret,
    #[token("%")] Percent => Percent,
    #[token("<<")] LtLt => LtLt,
    #[token(">>")] GtGt => GtGt,
    #[token(">>>")] GtGtGt => GtGtGt,
    #[token("+=")] PlusEq => PlusEq,
    #[token("-=")] MinusEq => MinusEq,
    #[token("*=")] StarEq => StarEq,
    #[token("/=")] SlashEq => SlashEq,
    #[token("&=")] AmpEq => AmpEq,
    #[token("|=")] PipeEq => PipeEq,
    #[token("^=")] CaretEq => CaretEq,
    #[token("%=")] PercentEq => PercentEq,
    #[token("<<=")] LtLtEq => LtLtEq,
    #[token(">>=")] GtGtEq => GtGtEq,
    #[token(">>>=")] GtGtGtEq => GtGtGtEq,
    #[token("_")] Underscore => Underscore,
}