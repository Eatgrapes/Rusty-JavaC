use javac_ast::JavaSyntaxKind;
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

struct Token {
    kind: JavaSyntaxKind,
    text: String,
    offset: usize,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    builder: GreenNodeBuilder<'static>,
    errors: Vec<ParseError>,
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
            tokens,
            pos: 0,
            builder: GreenNodeBuilder::new(),
            errors: Vec::new(),
        };

        parser.compilation_unit();
        let green_node = parser.builder.finish();

        Parse {
            green_node,
            errors: parser.errors,
        }
    }

    fn node(&mut self, kind: JavaSyntaxKind, f: impl FnOnce(&mut Parser)) {
        self.builder.start_node(kind.into());
        f(self);
        self.builder.finish_node();
    }

    fn kind(&self) -> JavaSyntaxKind {
        self.tokens.get(self.pos).map(|t| t.kind).unwrap_or(JavaSyntaxKind::Error)
    }

    fn look(&self, ahead: usize) -> JavaSyntaxKind {
        self.tokens.get(self.pos + ahead).map(|t| t.kind).unwrap_or(JavaSyntaxKind::Error)
    }

    fn at(&self, k: JavaSyntaxKind) -> bool { self.kind() == k }

    fn at_any(&self, ks: &[JavaSyntaxKind]) -> bool { ks.contains(&self.kind()) }

    fn bump(&mut self) {
        if self.pos < self.tokens.len() {
            let tok = &self.tokens[self.pos];
            self.builder.token(tok.kind.into(), tok.text.as_str());
            self.pos += 1;
        }
    }

    fn expect(&mut self, k: JavaSyntaxKind) {
        if self.at(k) { self.bump(); } else {
            self.err(format!("expected {:?}, got {:?}", k, self.kind()));
        }
    }

    fn eat(&mut self, k: JavaSyntaxKind) -> bool {
        if self.at(k) { self.bump(); true } else { false }
    }

    fn err(&mut self, msg: impl Into<String>) {
        let off = self.tokens.get(self.pos).map(|t| t.offset).unwrap_or(0);
        self.errors.push(ParseError { message: msg.into(), offset: off });
    }

    fn err_and_bump(&mut self, msg: impl Into<String>) {
        self.err(msg);
        self.bump();
    }

    fn compilation_unit(&mut self) {
        use JavaSyntaxKind::*;
        self.node(CompilationUnit, |p| {
            if p.at(PackageKw) { p.package_decl(); }
            while p.at(ImportKw) { p.import_decl(); }
            while p.at_any(&[ClassKw, InterfaceKw, EnumKw, RecordKw, At,
                PublicKw, ProtectedKw, PrivateKw, AbstractKw, FinalKw,
                StaticKw, StrictfpKw, SealedKw, NonSealedKw]) {
                p.type_decl();
            }
        });
    }

    fn package_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(PackageDecl, |p| {
            p.expect(PackageKw);
            p.qualified_name();
            p.expect(Semi);
        });
    }

    fn import_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ImportDecl, |p| {
            p.expect(ImportKw);
            p.eat(StaticKw);
            p.qualified_name();
            if p.eat(Star) {}
            p.expect(Semi);
        });
    }

    fn type_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.modifier_list();
        match self.kind() {
            ClassKw => self.class_decl(),
            InterfaceKw => self.interface_decl(),
            EnumKw => self.enum_decl(),
            RecordKw => self.record_decl(),
            At if self.look(1) == InterfaceKw => self.annotation_decl(),
            _ => self.err_and_bump("expected type declaration"),
        }
    }

    fn modifier_list(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ModifierList, |p| {
            let mods = [PublicKw, ProtectedKw, PrivateKw, AbstractKw, FinalKw,
                StaticKw, StrictfpKw, SealedKw, NonSealedKw, NativeKw,
                SynchronizedKw, TransientKw, VolatileKw, DefaultKw];
            loop {
                if p.at_any(&mods) { p.bump(); }
                else if p.at(At) { p.annotation(); }
                else { break; }
            }
        });
    }

    fn class_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ClassDecl, |p| {
            p.expect(ClassKw);
            p.expect(Ident);
            if p.at(Lt) { p.type_param_list(); }
            if p.eat(ExtendsKw) { p.type_(); }
            if p.eat(ImplementsKw) { p.type_list(); }
            if p.at(PermitsKw) { p.permits_clause(); }
            p.class_body();
        });
    }

    fn interface_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(InterfaceDecl, |p| {
            p.expect(InterfaceKw);
            p.expect(Ident);
            if p.at(Lt) { p.type_param_list(); }
            if p.eat(ExtendsKw) { p.type_list(); }
            if p.at(PermitsKw) { p.permits_clause(); }
            p.class_body();
        });
    }

    fn enum_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(EnumDecl, |p| {
            p.expect(EnumKw);
            p.expect(Ident);
            if p.at(Lt) { p.type_param_list(); }
            if p.eat(ImplementsKw) { p.type_list(); }
            p.enum_body();
        });
    }

    fn record_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(RecordDecl, |p| {
            p.expect(RecordKw);
            p.expect(Ident);
            if p.at(Lt) { p.type_param_list(); }
            p.record_component_list();
            if p.eat(ImplementsKw) { p.type_list(); }
            p.class_body();
        });
    }

    fn annotation_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(AnnotationDecl, |p| {
            p.expect(At);
            p.expect(InterfaceKw);
            p.expect(Ident);
            p.class_body();
        });
    }

    fn permits_clause(&mut self) {
        use JavaSyntaxKind::*;
        self.node(PermitsClause, |p| {
            p.expect(PermitsKw);
            p.type_list();
        });
    }

    fn class_body(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ClassBody, |p| {
            p.expect(LBrace);
            while !p.at(RBrace) && p.kind() != Error { p.class_member(); }
            p.expect(RBrace);
        });
    }

    fn enum_body(&mut self) {
        use JavaSyntaxKind::*;
        self.node(EnumBody, |p| {
            p.expect(LBrace);
            if !p.at(RBrace) { p.enum_constant_list(); }
            if p.eat(Semi) {
                while !p.at(RBrace) && p.kind() != Error { p.class_member(); }
            }
            p.expect(RBrace);
        });
    }

    fn enum_constant_list(&mut self) {
        use JavaSyntaxKind::*;
        self.node(EnumConstantList, |p| {
            loop {
                p.enum_constant();
                if !p.eat(Comma) { break; }
            }
        });
    }

    fn enum_constant(&mut self) {
        use JavaSyntaxKind::*;
        self.node(EnumConstant, |p| {
            p.modifier_list();
            p.expect(Ident);
            if p.at(LParen) { p.argument_list(); }
            if p.at(LBrace) { p.class_body(); }
        });
    }

    fn record_component_list(&mut self) {
        use JavaSyntaxKind::*;
        self.node(RecordComponentList, |p| {
            p.expect(LParen);
            while !p.at(RParen) && p.kind() != Error {
                p.node(RecordComponent, |p| {
                    p.modifier_list();
                    p.type_();
                    p.expect(Ident);
                });
                if !p.eat(Comma) { break; }
            }
            p.expect(RParen);
        });
    }

    fn class_member(&mut self) {
        use JavaSyntaxKind::*;
        if self.at(LBrace) {
            self.node(InstanceInit, |p| { p.block(); });
            return;
        }
        if self.at(StaticKw) && self.look(1) == LBrace {
            self.node(StaticInit, |p| { p.eat(StaticKw); p.block(); });
            return;
        }

        self.modifier_list();

        if self.at_any(&[ClassKw, InterfaceKw, EnumKw, RecordKw]) ||
           (self.at(At) && self.look(1) == InterfaceKw)
        {
            self.type_decl();
            return;
        }

        if self.is_constructor() {
            self.constructor_decl();
        } else if self.is_method_decl() {
            self.method_decl();
        } else {
            self.field_decl();
        }
    }

    fn is_constructor(&self) -> bool {
        let i = self.pos;
        i < self.tokens.len()
            && self.tokens[i].kind == JavaSyntaxKind::Ident
            && i + 1 < self.tokens.len()
            && (self.tokens[i + 1].kind == JavaSyntaxKind::LParen
                || self.tokens[i + 1].kind == JavaSyntaxKind::LBrace)
    }

    fn constructor_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ConstructorDecl, |p| {
            p.expect(Ident);
            if p.at(LParen) { p.formal_param_list(); }
            if p.at(ThrowsKw) { p.throws_clause(); }
            p.node(MethodBody, |p| { p.block(); });
        });
    }

    fn is_method_decl(&mut self) -> bool {
        let mut i = self.pos;
        while i < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::At {
            i += 1;
            if i < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::Ident { i += 1; }
            if i < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::LParen {
                let mut depth = 0;
                while i < self.tokens.len() {
                    match self.tokens[i].kind {
                        JavaSyntaxKind::LParen => depth += 1,
                        JavaSyntaxKind::RParen => { depth -= 1; if depth == 0 { i += 1; break; } }
                        _ => {}
                    }
                    i += 1;
                }
            }
        }
        let primitives = [JavaSyntaxKind::IntKw, JavaSyntaxKind::LongKw, JavaSyntaxKind::ShortKw,
            JavaSyntaxKind::ByteKw, JavaSyntaxKind::CharKw, JavaSyntaxKind::FloatKw,
            JavaSyntaxKind::DoubleKw, JavaSyntaxKind::BooleanKw, JavaSyntaxKind::VoidKw];
        if i < self.tokens.len() && primitives.contains(&self.tokens[i].kind) {
            i += 1;
        } else {
            while i < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::Ident {
                i += 1;
                if i < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::Lt {
                    let mut depth = 0;
                    while i < self.tokens.len() {
                        match self.tokens[i].kind {
                            JavaSyntaxKind::Lt => depth += 1,
                            JavaSyntaxKind::Gt => { depth -= 1; if depth == 0 { i += 1; break; } }
                            _ => {}
                        }
                        i += 1;
                    }
                }
                if i < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::Dot { i += 1; } else { break; }
            }
        }
        while i + 1 < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::LBrack && self.tokens[i + 1].kind == JavaSyntaxKind::RBrack {
            i += 2;
        }
        if i < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::Ident {
            i += 1;
            while i + 1 < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::LBrack && self.tokens[i + 1].kind == JavaSyntaxKind::RBrack {
                i += 2;
            }
        }
        i < self.tokens.len() && self.tokens[i].kind == JavaSyntaxKind::LParen
    }

    fn method_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(MethodDecl, |p| {
            p.type_();
            p.expect(Ident);
            p.formal_param_list();
            if p.at(ThrowsKw) { p.throws_clause(); }
            if p.eat(Semi) {
            } else {
                p.node(MethodBody, |p| { p.block(); });
            }
        });
    }

    fn field_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(FieldDecl, |p| {
            p.type_();
            p.node(VarDeclaratorList, |p| {
                loop {
                    p.node(VarDeclarator, |p| {
                        p.expect(Ident);
                        while p.eat(LBrack) { p.expect(RBrack); }
                        if p.eat(Eq) { p.expr(); }
                    });
                    if !p.eat(Comma) { break; }
                }
            });
            p.expect(Semi);
        });
    }

    fn formal_param_list(&mut self) {
        use JavaSyntaxKind::*;
        self.node(FormalParamList, |p| {
            p.expect(LParen);
            while !p.at(RParen) && p.kind() != Error {
                if p.at(ThisKw) {
                    p.node(ReceiverParam, |p| { p.bump(); });
                    break;
                }
                p.node(FormalParam, |p| {
                    p.modifier_list();
                    p.type_();
                    p.eat(Ellipsis);
                    p.expect(Ident);
                    while p.eat(LBrack) { p.expect(RBrack); }
                });
                if !p.eat(Comma) { break; }
            }
            p.expect(RParen);
        });
    }

    fn throws_clause(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ThrowsClause, |p| {
            p.expect(ThrowsKw);
            p.node(ExceptionTypeList, |p| { p.type_list(); });
        });
    }

    fn type_(&mut self) {
        use JavaSyntaxKind::*;
        self.node(Type, |p| {
            if p.at(VarKw) {
                p.bump();
            } else if p.at_any(&[IntKw, LongKw, ShortKw, ByteKw, CharKw,
                FloatKw, DoubleKw, BooleanKw, VoidKw]) {
                p.node(PrimitiveType, |p| { p.bump(); });
            } else {
                p.class_type();
            }
            while p.at(LBrack) {
                p.bump();
                p.expect(RBrack);
            }
        });
    }

    fn class_type(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ClassType, |p| {
            p.expect(Ident);
            if p.at(Lt) { p.type_arg_list(); }
            while p.eat(Dot) {
                p.node(ClassTypeSegment, |p| {
                    p.expect(Ident);
                    if p.at(Lt) { p.type_arg_list(); }
                });
            }
        });
    }

    fn type_arg_list(&mut self) {
        use JavaSyntaxKind::*;
        self.node(TypeArgList, |p| {
            p.expect(Lt);
            while !p.at(Gt) && p.kind() != Error {
                if p.at(Question) {
                    p.node(WildcardType, |p| {
                        p.bump();
                        if p.eat(ExtendsKw) || p.eat(SuperKw) { p.type_(); }
                    });
                } else {
                    p.type_();
                }
                if !p.eat(Comma) { break; }
            }
            p.expect(Gt);
        });
    }

    fn type_param_list(&mut self) {
        use JavaSyntaxKind::*;
        self.node(TypeParamList, |p| {
            p.expect(Lt);
            while !p.at(Gt) && p.kind() != Error {
                p.node(TypeParam, |p| {
                    p.expect(Ident);
                    if p.eat(ExtendsKw) {
                        p.node(TypeBound, |p| { p.type_list(); });
                    }
                });
                if !p.eat(Comma) { break; }
            }
            p.expect(Gt);
        });
    }

    fn type_list(&mut self) {
        self.type_();
        while self.eat(JavaSyntaxKind::Comma) { self.type_(); }
    }

    fn qualified_name(&mut self) {
        use JavaSyntaxKind::*;
        self.node(QualifiedName, |p| {
            p.expect(Ident);
            while p.eat(Dot) { p.expect(Ident); }
        });
    }

    fn annotation(&mut self) {
        use JavaSyntaxKind::*;
        self.node(Annotation, |p| {
            p.expect(At);
            p.qualified_name();
            if p.at(LParen) {
                p.argument_list();
            }
        });
    }

    fn block(&mut self) {
        use JavaSyntaxKind::*;
        self.node(Block, |p| {
            p.expect(LBrace);
            while !p.at(RBrace) && p.kind() != Error { p.stmt(); }
            p.expect(RBrace);
        });
    }

    fn stmt(&mut self) {
        use JavaSyntaxKind::*;
        match self.kind() {
            LBrace => self.block(),
            IfKw => self.if_stmt(),
            ForKw => self.for_stmt(),
            WhileKw => self.while_stmt(),
            DoKw => self.do_stmt(),
            SwitchKw => self.switch_expr(),
            TryKw => self.try_stmt(),
            ReturnKw => self.return_stmt(),
            ThrowKw => self.throw_stmt(),
            BreakKw => self.break_stmt(),
            ContinueKw => self.continue_stmt(),
            SynchronizedKw => self.synchronized_stmt(),
            AssertKw => self.assert_stmt(),
            YieldKw => self.yield_stmt(),
            Semi => { self.node(EmptyStmt, |p| { p.bump(); }); }
            _ => self.expr_or_local_decl(),
        }
    }

    fn if_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(IfStmt, |p| {
            p.expect(IfKw);
            p.expect(LParen); p.expr(); p.expect(RParen);
            p.stmt();
            if p.eat(ElseKw) { p.stmt(); }
        });
    }

    fn for_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ForStmt, |p| {
            p.expect(ForKw);
            p.expect(LParen);
            if p.is_foreach() {
                p.for_each();
            } else {
                p.for_init();
                p.expect(Semi);
                if !p.at(Semi) { p.expr(); }
                p.expect(Semi);
                if !p.at(RParen) { p.expr_list(); }
                p.expect(RParen);
                p.stmt();
            }
        });
    }

    fn is_foreach(&mut self) -> bool {
        let mut i = self.pos;
        let mut depth = 1i32;
        while i < self.tokens.len() {
            match self.tokens[i].kind {
                JavaSyntaxKind::LParen => depth += 1,
                JavaSyntaxKind::RParen => { depth -= 1; if depth == 0 { return false; } }
                JavaSyntaxKind::Colon if depth == 1 => return true,
                _ => {}
            }
            i += 1;
        }
      false
    }

    fn for_each(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ForEach, |p| {
            p.modifier_list();
            p.type_();
            p.expect(Ident);
            p.expect(Colon);
            p.expr();
            p.expect(RParen);
            p.stmt();
        });
    }

    fn for_init(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ForInit, |p| {
            if p.at(Semi) { return; }
            if p.is_local_var_decl() {
                p.type_();
                p.node(VarDeclaratorList, |p| {
                    loop {
                        p.node(VarDeclarator, |p| {
                            p.expect(Ident);
                            while p.eat(LBrack) { p.expect(RBrack); }
                            if p.eat(Eq) { p.expr(); }
                        });
                        if !p.eat(Comma) { break; }
                    }
                });
            } else {
                p.expr();
                if p.eat(Comma) { p.expr_list(); }
            }
        });
    }

    fn while_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(WhileStmt, |p| {
            p.expect(WhileKw);
            p.expect(LParen); p.expr(); p.expect(RParen);
            p.stmt();
        });
    }

    fn do_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(DoStmt, |p| {
            p.expect(DoKw);
            p.stmt();
            p.expect(WhileKw);
            p.expect(LParen); p.expr(); p.expect(RParen);
            p.expect(Semi);
        });
    }

    fn switch_expr(&mut self) {
        use JavaSyntaxKind::*;
        self.node(SwitchStmt, |p| {
            p.expect(SwitchKw);
            p.expect(LParen); p.expr(); p.expect(RParen);
            p.node(SwitchBlock, |p| {
                p.expect(LBrace);
                while !p.at(RBrace) && p.kind() != Error {
                    p.switch_label();
                }
                p.expect(RBrace);
            });
        });
    }

    fn switch_label(&mut self) {
        use JavaSyntaxKind::*;
        self.node(SwitchLabel, |p| {
            if p.eat(CaseKw) {
                p.expr();
                if p.eat(Colon) {
                    while !p.at_any(&[CaseKw, DefaultKw, RBrace]) && p.kind() != Error {
                        p.stmt();
                    }
                } else {
                    p.expect(Arrow);
                    p.node(SwitchRule, |p| {
                        if p.at(ThrowKw) { p.stmt(); }
                        else if p.at(LBrace) { p.block(); }
                        else { p.expr(); p.eat(Semi); }
                    });
                }
            } else {
                p.expect(DefaultKw);
                if p.eat(Colon) {
                    while !p.at_any(&[CaseKw, RBrace]) && p.kind() != Error {
                        p.stmt();
                    }
                } else {
                    p.expect(Arrow);
                    p.node(SwitchRule, |p| {
                        if p.at(ThrowKw) { p.stmt(); }
                        else if p.at(LBrace) { p.block(); }
                        else { p.expr(); p.eat(Semi); }
                    });
                }
            }
        });
    }

    fn try_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(TryStmt, |p| {
            p.expect(TryKw);
            if p.at(LParen) {
                p.node(TryWithResources, |p| {
                    p.expect(LParen);
                    while !p.at(RParen) && p.kind() != Error {
                        p.node(Resource, |p| {
                            p.modifier_list();
                            p.type_();
                            p.expect(Ident);
                            if p.eat(Eq) { p.expr(); }
                        });
                        if !p.eat(Semi) { break; }
                    }
                    p.expect(RParen);
                });
            }
            p.block();
            while p.at(CatchKw) { p.catch_clause(); }
            if p.at(FinallyKw) {
                p.node(FinallyClause, |p| { p.expect(FinallyKw); p.block(); });
            }
        });
    }

    fn catch_clause(&mut self) {
        use JavaSyntaxKind::*;
        self.node(CatchClause, |p| {
            p.expect(CatchKw);
            p.expect(LParen);
            p.modifier_list();
            p.type_();
            p.expect(Ident);
            p.expect(RParen);
            p.block();
        });
    }

    fn return_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ReturnStmt, |p| {
            p.expect(ReturnKw);
            if !p.at(Semi) { p.expr(); }
            p.expect(Semi);
        });
    }

    fn throw_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ThrowStmt, |p| {
            p.expect(ThrowKw);
            p.expr();
            p.expect(Semi);
        });
    }

    fn break_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(BreakStmt, |p| {
            p.expect(BreakKw);
            if p.at(Ident) { p.bump(); }
            p.expect(Semi);
        });
    }

    fn continue_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ContinueStmt, |p| {
            p.expect(ContinueKw);
            if p.at(Ident) { p.bump(); }
            p.expect(Semi);
        });
    }

    fn synchronized_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(SynchronizedStmt, |p| {
            p.expect(SynchronizedKw);
            p.expect(LParen); p.expr(); p.expect(RParen);
            p.block();
        });
    }

    fn assert_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(AssertStmt, |p| {
            p.expect(AssertKw);
            p.expr();
            if p.eat(Colon) { p.expr(); }
            p.expect(Semi);
        });
    }

    fn yield_stmt(&mut self) {
        use JavaSyntaxKind::*;
        self.node(YieldStmt, |p| {
            p.expect(YieldKw);
            p.expr();
            p.expect(Semi);
        });
    }

    fn is_local_var_decl(&self) -> bool {
        use JavaSyntaxKind::*;
        let primitives = [IntKw, LongKw, ShortKw, ByteKw, CharKw,
            FloatKw, DoubleKw, BooleanKw, VoidKw, VarKw];
        if self.at_any(&primitives) { return true; }
        if !self.at(Ident) { return false; }
        let mut i = self.pos;
        while i < self.tokens.len() && self.tokens[i].kind == Ident {
            i += 1;
            if i < self.tokens.len() && self.tokens[i].kind == Lt {
                let mut depth = 0;
                while i < self.tokens.len() {
                    match self.tokens[i].kind {
                        Lt => depth += 1,
                        Gt => { depth -= 1; if depth == 0 { i += 1; break; } }
                        _ => {}
                    }
                    i += 1;
                }
            }
            if i < self.tokens.len() && self.tokens[i].kind == Dot { i += 1; } else { break; }
        }
        while i + 1 < self.tokens.len() && self.tokens[i].kind == LBrack && self.tokens[i + 1].kind == RBrack {
            i += 2;
        }
        i < self.tokens.len() && self.tokens[i].kind == Ident
    }

    fn local_var_decl(&mut self) {
        use JavaSyntaxKind::*;
        self.node(LocalVarDecl, |p| {
            p.type_();
            p.node(VarDeclaratorList, |p| {
                loop {
                    p.node(VarDeclarator, |p| {
                        p.expect(Ident);
                        while p.eat(LBrack) { p.expect(RBrack); }
                        if p.eat(Eq) { p.expr(); }
                    });
                    if !p.eat(Comma) { break; }
                }
            });
            p.expect(Semi);
        });
    }

    fn expr_or_local_decl(&mut self) {
        use JavaSyntaxKind::*;
        if self.is_local_var_decl() {
            self.local_var_decl();
        } else {
            self.node(ExprStmt, |p| {
                p.expr();
                p.expect(Semi);
            });
        }
    }

    fn expr(&mut self) { self.assignment_expr(); }

    fn assignment_expr(&mut self) {
        use JavaSyntaxKind::*;
        self.ternary_expr();
        if self.at_any(&[Eq, PlusEq, MinusEq, StarEq, SlashEq, AmpEq, PipeEq, CaretEq, PercentEq, LtLtEq, GtGtEq, GtGtGtEq]) {
            self.bump();
            self.assignment_expr();
        }
    }

    fn ternary_expr(&mut self) {
        use JavaSyntaxKind::*;
        self.binary_expr(0);
        if self.eat(Question) {
            self.expr();
            self.expect(Colon);
            self.ternary_expr();
        }
    }

    fn binary_expr(&mut self, min_prec: usize) {
        self.unary_expr();
        loop {
            let prec = self.binop_prec();
            if prec == 0 || prec < min_prec { break; }
            self.bump();
            self.binary_expr(prec + 1);
        }
    }

    fn binop_prec(&self) -> usize {
        use JavaSyntaxKind::*;
        match self.kind() {
            PipePipe => 1,
            AmpAmp => 2,
            Pipe => 3,
            Caret => 4,
            Amp => 5,
            EqEq | Neq => 6,
            Lt | Gt | Le | Ge | InstanceofKw => 7,
            LtLt | GtGt | GtGtGt => 8,
            Plus | Minus => 9,
            Star | Slash | Percent => 10,
            _ => 0,
        }
    }

    fn unary_expr(&mut self) {
        use JavaSyntaxKind::*;
        match self.kind() {
            Plus | Minus => { self.bump(); self.unary_expr(); }
            Inc | Dec => { self.bump(); self.unary_expr(); }
            Tilde | Bang => { self.bump(); self.unary_expr(); }
            _ => { self.cast_or_postfix_expr(); }
        }
    }

    fn cast_or_postfix_expr(&mut self) {
        use JavaSyntaxKind::*;
        if self.at(LParen) {
            if self.is_cast() {
                self.node(CastExpr, |p| {
                    p.expect(LParen);
                    p.type_();
                    p.expect(RParen);
                    p.unary_expr();
                });
                self.postfix_suffix();
                return;
            }
        }
        self.primary_expr();
        self.postfix_suffix();
    }

    fn is_cast(&mut self) -> bool {
        use JavaSyntaxKind::*;
        if !self.at(LParen) { return false; }
        let mut i = self.pos + 1;
        let primitives = [IntKw, LongKw, ShortKw, ByteKw, CharKw, FloatKw, DoubleKw, BooleanKw];
        if i < self.tokens.len() && primitives.contains(&self.tokens[i].kind) {
            i += 1;
            while i + 1 < self.tokens.len() && self.tokens[i].kind == LBrack && self.tokens[i + 1].kind == RBrack {
                i += 2;
            }
            return i < self.tokens.len() && self.tokens[i].kind == RParen;
        }
        if i < self.tokens.len() && self.tokens[i].kind == Ident {
            while i < self.tokens.len() && self.tokens[i].kind == Ident {
                i += 1;
                if i < self.tokens.len() && self.tokens[i].kind == Lt {
                    let mut depth = 0;
                    while i < self.tokens.len() {
                        match self.tokens[i].kind {
                            Lt => depth += 1,
                            Gt => { depth -= 1; if depth == 0 { i += 1; break; } }
                            _ => {}
                        }
                        i += 1;
                    }
                }
                if i < self.tokens.len() && self.tokens[i].kind == Dot { i += 1; } else { break; }
            }
            while i + 1 < self.tokens.len() && self.tokens[i].kind == LBrack && self.tokens[i + 1].kind == RBrack {
                i += 2;
            }
            if i < self.tokens.len() && self.tokens[i].kind == RParen {
                return true;
            }
        }
        false
    }

    fn postfix_suffix(&mut self) {
        use JavaSyntaxKind::*;
        loop {
            match self.kind() {
                Dot => {
                    self.bump();
                    if self.at(NewKw) {
                        self.new_expr();
                    } else {
                        self.expect(Ident);
                    }
                }
                LBrack => {
                    self.bump();
                    self.expr();
                    self.expect(RBrack);
                }
                LParen => { self.argument_list(); }
                Inc | Dec => { self.bump(); break; }
                _ => break,
            }
        }
    }

    fn primary_expr(&mut self) {
        use JavaSyntaxKind::*;
        match self.kind() {
            IntLiteral | LongLiteral | FloatLiteral | DoubleLiteral
            | CharLiteral | StringLiteral | TextBlockLiteral
            | TrueKw | FalseKw | NullKw => {
                self.node(Literal, |p| { p.bump(); });
            }
            ThisKw => { self.node(ThisExpr, |p| { p.bump(); }); }
            SuperKw => { self.node(SuperExpr, |p| { p.bump(); }); }
            NewKw => { self.new_expr(); }
            SwitchKw => { self.switch_expr(); }
            LParen => {
                self.node(ParenExpr, |p| {
                    p.bump();
                    p.expr();
                    p.expect(RParen);
                });
            }
            Ident => { self.name_expr(); }
            _ => { self.err_and_bump(format!("unexpected token in expression: {:?}", self.kind())); }
        }
    }

    fn name_expr(&mut self) {
        use JavaSyntaxKind::*;
        self.node(Name, |p| {
            p.expect(Ident);
            while p.eat(Dot) {
                p.node(MemberSelect, |p| { p.expect(Ident); });
            }
        });
    }

    fn new_expr(&mut self) {
        use JavaSyntaxKind::*;
        self.node(NewExpr, |p| {
            p.expect(NewKw);
            p.type_no_array();
            if p.at(LBrack) {
                p.bump();
                if !p.at(RBrack) { p.expr(); }
                p.expect(RBrack);
                while p.eat(LBrack) {
                    if !p.at(RBrack) { p.expr(); }
                    p.expect(RBrack);
                }
                if p.at(LBrace) { p.array_init(); }
            } else {
                p.argument_list();
                if p.at(LBrace) { p.class_body(); }
            }
        });
    }

    fn type_no_array(&mut self) {
        use JavaSyntaxKind::*;
        self.node(Type, |p| {
            if p.at(VarKw) {
                p.bump();
            } else if p.at_any(&[IntKw, LongKw, ShortKw, ByteKw, CharKw,
                FloatKw, DoubleKw, BooleanKw, VoidKw]) {
                p.node(PrimitiveType, |p| { p.bump(); });
            } else {
                p.class_type();
            }
        });
    }

    fn array_init(&mut self) {
        use JavaSyntaxKind::*;
        self.node(ArrayInit, |p| {
            p.expect(LBrace);
            while !p.at(RBrace) && p.kind() != Error {
                p.expr();
                if !p.eat(Comma) { break; }
            }
            p.eat(Comma);
            p.expect(RBrace);
        });
    }

    fn argument_list(&mut self) {
        use JavaSyntaxKind::*;
        self.expect(LParen);
        if !self.at(RParen) {
            self.expr();
            while self.eat(Comma) { self.expr(); }
        }
        self.expect(RParen);
    }

    fn expr_list(&mut self) {
        self.expr();
        while self.eat(JavaSyntaxKind::Comma) { self.expr(); }
    }
}