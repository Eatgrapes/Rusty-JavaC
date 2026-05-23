use crate::parser::{JavaSyntaxKind, Parser};
use crate::parser::{stmt, ty, type_decl};

pub(crate) fn expr(p: &mut Parser) {
    assignment_expr(p);
}

pub(crate) fn assignment_expr(p: &mut Parser) {
    use JavaSyntaxKind::*;
    ternary_expr(p);
    if p.at_any(&[
        Eq, PlusEq, MinusEq, StarEq, SlashEq, AmpEq, PipeEq, CaretEq, PercentEq, LtLtEq, GtGtEq,
        GtGtGtEq,
    ]) {
        p.bump();
        assignment_expr(p);
    }
}

pub(crate) fn ternary_expr(p: &mut Parser) {
    use JavaSyntaxKind::*;
    binary_expr(p, 0);
    if p.eat(Question) {
        expr(p);
        p.expect(Colon);
        ternary_expr(p);
    }
}

pub(crate) fn binary_expr(p: &mut Parser, min_prec: usize) {
    unary_expr(p);
    loop {
        let prec = binop_prec(p);
        if prec == 0 || prec < min_prec {
            break;
        }
        let op = p.kind();
        p.bump();
        if op == JavaSyntaxKind::InstanceofKw {
            ty::type_(p);
            if p.at(JavaSyntaxKind::Ident) {
                p.bump();
            }
        } else {
            binary_expr(p, prec + 1);
        }
    }
}

pub(crate) fn binop_prec(p: &Parser) -> usize {
    use JavaSyntaxKind::*;
    match p.kind() {
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

pub(crate) fn unary_expr(p: &mut Parser) {
    use JavaSyntaxKind::*;
    match p.kind() {
        Plus | Minus => {
            p.bump();
            unary_expr(p);
        }
        Inc | Dec => {
            p.bump();
            unary_expr(p);
        }
        Tilde | Bang => {
            p.bump();
            unary_expr(p);
        }
        _ => {
            cast_or_postfix_expr(p);
        }
    }
}

pub(crate) fn cast_or_postfix_expr(p: &mut Parser) {
    use JavaSyntaxKind::*;
    if p.at(LParen) {
        if is_cast(p) {
            let m = p.start();
            p.expect(LParen);
            ty::type_(p);
            p.expect(RParen);
            unary_expr(p);
            m.complete(p, CastExpr);
            postfix_suffix(p);
            return;
        }
    }
    primary_expr(p);
    postfix_suffix(p);
}

pub(crate) fn is_cast(p: &Parser) -> bool {
    use JavaSyntaxKind::*;
    let mut la = p.lookahead();
    if !la.eat(LParen) {
        return false;
    }
    la.skip_type();
    la.skip_array_dims();
    la.at(RParen)
}

pub(crate) fn postfix_suffix(p: &mut Parser) {
    use JavaSyntaxKind::*;
    loop {
        match p.kind() {
            Dot => {
                p.bump();
                if p.at(NewKw) {
                    new_expr(p);
                } else {
                    p.expect(Ident);
                }
            }
            LBrack => {
                p.bump();
                expr(p);
                p.expect(RBrack);
            }
            LParen => {
                argument_list(p);
            }
            Inc | Dec => {
                p.bump();
                break;
            }
            _ => break,
        }
    }
}

pub(crate) fn primary_expr(p: &mut Parser) {
    use JavaSyntaxKind::*;
    match p.kind() {
        IntLiteral | LongLiteral | FloatLiteral | DoubleLiteral | CharLiteral | StringLiteral
        | TextBlockLiteral | TrueKw | FalseKw | NullKw => {
            let m = p.start();
            p.bump();
            m.complete(p, Literal);
        }
        ThisKw => {
            let m = p.start();
            p.bump();
            m.complete(p, ThisExpr);
        }
        SuperKw => {
            let m = p.start();
            p.bump();
            m.complete(p, SuperExpr);
        }
        NewKw => {
            new_expr(p);
        }
        SwitchKw => {
            stmt::switch_expr(p);
        }
        LParen => {
            let m = p.start();
            p.bump();
            expr(p);
            p.expect(RParen);
            m.complete(p, ParenExpr);
        }
        Ident => {
            name_expr(p);
        }
        _ => {
            p.err_and_bump(format!("unexpected token in expression: {:?}", p.kind()));
        }
    }
}

pub(crate) fn name_expr(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(Ident);
    while p.eat(Dot) {
        let sm = p.start();
        p.expect(Ident);
        sm.complete(p, MemberSelect);
    }
    m.complete(p, Name);
}

pub(crate) fn new_expr(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(NewKw);
    ty::type_no_array(p);
    if p.at(LBrack) {
        p.bump();
        if !p.at(RBrack) {
            expr(p);
        }
        p.expect(RBrack);
        while p.eat(LBrack) {
            if !p.at(RBrack) {
                expr(p);
            }
            p.expect(RBrack);
        }
        if p.at(LBrace) {
            array_init(p);
        }
    } else {
        argument_list(p);
        if p.at(LBrace) {
            type_decl::class_body(p);
        }
    }
    m.complete(p, NewExpr);
}

pub(crate) fn array_init(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(LBrace);
    while !p.at(RBrace) && p.kind() != Error {
        expr(p);
        if !p.eat(Comma) {
            break;
        }
    }
    p.eat(Comma);
    p.expect(RBrace);
    m.complete(p, ArrayInit);
}

pub(crate) fn argument_list(p: &mut Parser) {
    use JavaSyntaxKind::*;
    p.expect(LParen);
    if !p.at(RParen) {
        expr(p);
        while p.eat(Comma) {
            expr(p);
        }
    }
    p.expect(RParen);
}

pub(crate) fn expr_list(p: &mut Parser) {
    expr(p);
    while p.eat(JavaSyntaxKind::Comma) {
        expr(p);
    }
}
