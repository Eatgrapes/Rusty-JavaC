use crate::parser::{Parser, JavaSyntaxKind};
use crate::parser::{top_level, ty, member, expr};

pub(crate) fn type_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    top_level::modifier_list(p);
    match p.kind() {
        ClassKw => class_decl(p),
        InterfaceKw => interface_decl(p),
        EnumKw => enum_decl(p),
        RecordKw => record_decl(p),
        At if p.look(1) == InterfaceKw => annotation_decl(p),
        _ => p.err_and_bump("expected type declaration"),
    }
}

pub(crate) fn class_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(ClassKw);
    p.expect(Ident);
    if p.at(Lt) { ty::type_param_list(p); }
    if p.eat(ExtendsKw) { ty::type_(p); }
    if p.eat(ImplementsKw) { ty::type_list(p); }
    if p.at(PermitsKw) { permits_clause(p); }
    class_body(p);
    m.complete(p, ClassDecl);
}

pub(crate) fn interface_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(InterfaceKw);
    p.expect(Ident);
    if p.at(Lt) { ty::type_param_list(p); }
    if p.eat(ExtendsKw) { ty::type_list(p); }
    if p.at(PermitsKw) { permits_clause(p); }
    class_body(p);
    m.complete(p, InterfaceDecl);
}

pub(crate) fn enum_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(EnumKw);
    p.expect(Ident);
    if p.at(Lt) { ty::type_param_list(p); }
    if p.eat(ImplementsKw) { ty::type_list(p); }
    enum_body(p);
    m.complete(p, EnumDecl);
}

pub(crate) fn record_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(RecordKw);
    p.expect(Ident);
    if p.at(Lt) { ty::type_param_list(p); }
    member::record_component_list(p);
    if p.eat(ImplementsKw) { ty::type_list(p); }
    class_body(p);
    m.complete(p, RecordDecl);
}

pub(crate) fn annotation_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(At);
    p.expect(InterfaceKw);
    p.expect(Ident);
    class_body(p);
    m.complete(p, AnnotationDecl);
}

pub(crate) fn permits_clause(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(PermitsKw);
    ty::type_list(p);
    m.complete(p, PermitsClause);
}

pub(crate) fn class_body(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(LBrace);
    while !p.at(RBrace) && p.kind() != Error { member::class_member(p); }
    p.expect(RBrace);
    m.complete(p, ClassBody);
}

pub(crate) fn enum_body(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(LBrace);
    if !p.at(RBrace) { enum_constant_list(p); }
    if p.eat(Semi) {
        while !p.at(RBrace) && p.kind() != Error { member::class_member(p); }
    }
    p.expect(RBrace);
    m.complete(p, EnumBody);
}

pub(crate) fn enum_constant_list(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    loop {
        enum_constant(p);
        if !p.eat(Comma) { break; }
    }
    m.complete(p, EnumConstantList);
}

pub(crate) fn enum_constant(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    top_level::modifier_list(p);
    p.expect(Ident);
    if p.at(LParen) { expr::argument_list(p); }
    if p.at(LBrace) { class_body(p); }
    m.complete(p, EnumConstant);
}
