use crate::parser::{Parser, JavaSyntaxKind};

pub(crate) fn type_(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    if p.at(VarKw) {
        p.bump();
    } else if p.at_any(&[IntKw, LongKw, ShortKw, ByteKw, CharKw,
        FloatKw, DoubleKw, BooleanKw, VoidKw]) {
        let pm = p.start();
        p.bump();
        pm.complete(p, PrimitiveType);
    } else {
        class_type(p);
    }
    while p.at(LBrack) {
        p.bump();
        p.expect(RBrack);
    }
    m.complete(p, Type);
}

pub(crate) fn type_no_array(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    if p.at(VarKw) {
        p.bump();
    } else if p.at_any(&[IntKw, LongKw, ShortKw, ByteKw, CharKw,
        FloatKw, DoubleKw, BooleanKw, VoidKw]) {
        let pm = p.start();
        p.bump();
        pm.complete(p, PrimitiveType);
    } else {
        class_type(p);
    }
    m.complete(p, Type);
}

pub(crate) fn class_type(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(Ident);
    if p.at(Lt) { type_arg_list(p); }
    while p.eat(Dot) {
        let sm = p.start();
        p.expect(Ident);
        if p.at(Lt) { type_arg_list(p); }
        sm.complete(p, ClassTypeSegment);
    }
    m.complete(p, ClassType);
}

pub(crate) fn type_arg_list(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(Lt);
    while !p.at(Gt) && p.kind() != Error {
        if p.at(Question) {
            let wm = p.start();
            p.bump();
            if p.eat(ExtendsKw) || p.eat(SuperKw) { type_(p); }
            wm.complete(p, WildcardType);
        } else {
            type_(p);
        }
        if !p.eat(Comma) { break; }
    }
    p.expect(Gt);
    m.complete(p, TypeArgList);
}

pub(crate) fn type_param_list(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(Lt);
    while !p.at(Gt) && p.kind() != Error {
        let pm = p.start();
        p.expect(Ident);
        if p.eat(ExtendsKw) {
            let bm = p.start();
            type_list(p);
            bm.complete(p, TypeBound);
        }
        pm.complete(p, TypeParam);
        if !p.eat(Comma) { break; }
    }
    p.expect(Gt);
    m.complete(p, TypeParamList);
}

pub(crate) fn type_list(p: &mut Parser) {
    type_(p);
    while p.eat(JavaSyntaxKind::Comma) { type_(p); }
}
