use crate::parser::JavaSyntaxKind;
use crate::parser::Parser;
use crate::parser::{expr, stmt, top_level, ty, type_decl};

pub(crate) fn record_component_list(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(LParen);
    while !p.at(RParen) && p.kind() != Error {
        let cm = p.start();
        top_level::modifier_list(p);
        ty::type_(p);
        p.expect(Ident);
        cm.complete(p, RecordComponent);
        if !p.eat(Comma) {
            break;
        }
    }
    p.expect(RParen);
    m.complete(p, RecordComponentList);
}

pub(crate) fn class_member(p: &mut Parser) {
    use JavaSyntaxKind::*;
    if p.at(LBrace) {
        let m = p.start();
        stmt::block(p);
        m.complete(p, InstanceInit);
        return;
    }
    if p.at(StaticKw) && p.look(1) == LBrace {
        let m = p.start();
        p.eat(StaticKw);
        stmt::block(p);
        m.complete(p, StaticInit);
        return;
    }

    top_level::modifier_list(p);

    if p.at_any(&[ClassKw, InterfaceKw, EnumKw, RecordKw]) || (p.at(At) && p.look(1) == InterfaceKw)
    {
        type_decl::type_decl(p);
        return;
    }

    if is_constructor(p) {
        constructor_decl(p);
    } else if is_method_decl(p) {
        method_decl(p);
    } else {
        field_decl(p);
    }
}

pub(crate) fn is_constructor(p: &Parser) -> bool {
    let mut la = p.lookahead();
    la.at(JavaSyntaxKind::Ident) && {
        la.advance();
        la.at(JavaSyntaxKind::LParen) || la.at(JavaSyntaxKind::LBrace)
    }
}

pub(crate) fn constructor_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(Ident);
    if p.at(LParen) {
        formal_param_list(p);
    }
    if p.at(ThrowsKw) {
        throws_clause(p);
    }
    let bm = p.start();
    stmt::block(p);
    bm.complete(p, MethodBody);
    m.complete(p, ConstructorDecl);
}

pub(crate) fn is_method_decl(p: &Parser) -> bool {
    use JavaSyntaxKind::*;
    let mut la = p.lookahead();
    la.skip_annotations();
    la.skip_type();
    la.skip_array_dims();
    // After type + dims, expect: name dims '('
    if !la.eat(Ident) {
        return false;
    }
    la.skip_array_dims();
    la.at(LParen)
}

pub(crate) fn method_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    ty::type_(p);
    p.expect(Ident);
    formal_param_list(p);
    if p.at(ThrowsKw) {
        throws_clause(p);
    }
    if p.eat(Semi) {
    } else {
        let bm = p.start();
        stmt::block(p);
        bm.complete(p, MethodBody);
    }
    m.complete(p, MethodDecl);
}

pub(crate) fn field_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    ty::type_(p);
    let vlm = p.start();
    loop {
        let vm = p.start();
        p.expect(Ident);
        while p.eat(LBrack) {
            p.expect(RBrack);
        }
        if p.eat(Eq) {
            expr::expr(p);
        }
        vm.complete(p, VarDeclarator);
        if !p.eat(Comma) {
            break;
        }
    }
    vlm.complete(p, VarDeclaratorList);
    p.expect(Semi);
    m.complete(p, FieldDecl);
}

pub(crate) fn formal_param_list(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(LParen);
    while !p.at(RParen) && p.kind() != Error {
        if p.at(ThisKw) {
            let rm = p.start();
            p.bump();
            rm.complete(p, ReceiverParam);
            break;
        }
        let fm = p.start();
        top_level::modifier_list(p);
        ty::type_(p);
        p.eat(Ellipsis);
        p.expect(Ident);
        while p.eat(LBrack) {
            p.expect(RBrack);
        }
        fm.complete(p, FormalParam);
        if !p.eat(Comma) {
            break;
        }
    }
    p.expect(RParen);
    m.complete(p, FormalParamList);
}

pub(crate) fn throws_clause(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(ThrowsKw);
    let em = p.start();
    ty::type_list(p);
    em.complete(p, ExceptionTypeList);
    m.complete(p, ThrowsClause);
}
