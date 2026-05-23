use crate::parser::{JavaSyntaxKind, Parser};
use crate::parser::{expr, top_level, ty};

pub(crate) fn block(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(LBrace);
    while !p.at(RBrace) && p.kind() != Error {
        stmt(p);
    }
    p.expect(RBrace);
    m.complete(p, Block);
}

pub(crate) fn stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    match p.kind() {
        LBrace => block(p),
        IfKw => if_stmt(p),
        ForKw => for_stmt(p),
        WhileKw => while_stmt(p),
        DoKw => do_stmt(p),
        SwitchKw => switch_expr(p),
        TryKw => try_stmt(p),
        ReturnKw => return_stmt(p),
        ThrowKw => throw_stmt(p),
        BreakKw => break_stmt(p),
        ContinueKw => continue_stmt(p),
        SynchronizedKw => synchronized_stmt(p),
        AssertKw => assert_stmt(p),
        YieldKw => yield_stmt(p),
        Semi => {
            let m = p.start();
            p.bump();
            m.complete(p, EmptyStmt);
        }
        _ => expr_or_local_decl(p),
    }
}

pub(crate) fn if_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(IfKw);
    p.expect(LParen);
    expr::expr(p);
    p.expect(RParen);
    stmt(p);
    if p.eat(ElseKw) {
        stmt(p);
    }
    m.complete(p, IfStmt);
}

pub(crate) fn for_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(ForKw);
    p.expect(LParen);
    if is_foreach(p) {
        for_each(p);
    } else {
        for_init(p);
        p.expect(Semi);
        if !p.at(Semi) {
            expr::expr(p);
        }
        p.expect(Semi);
        if !p.at(RParen) {
            expr::expr_list(p);
        }
        p.expect(RParen);
        stmt(p);
    }
    m.complete(p, ForStmt);
}

pub(crate) fn is_foreach(p: &Parser) -> bool {
    use JavaSyntaxKind::*;
    let mut la = p.lookahead();
    let mut depth = 1usize;
    while la.kind() != Error {
        match la.kind() {
            LParen => depth += 1,
            RParen => {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
            Colon if depth == 1 => return true,
            _ => {}
        }
        la.advance();
    }
    false
}

pub(crate) fn for_each(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    top_level::modifier_list(p);
    ty::type_(p);
    p.expect(Ident);
    p.expect(Colon);
    expr::expr(p);
    p.expect(RParen);
    stmt(p);
    m.complete(p, ForEach);
}

pub(crate) fn for_init(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    if p.at(Semi) {
        m.abandon(p);
        return;
    }
    if is_local_var_decl(p) {
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
    } else {
        expr::expr(p);
        if p.eat(Comma) {
            expr::expr_list(p);
        }
    }
    m.complete(p, ForInit);
}

pub(crate) fn while_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(WhileKw);
    p.expect(LParen);
    expr::expr(p);
    p.expect(RParen);
    stmt(p);
    m.complete(p, WhileStmt);
}

pub(crate) fn do_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(DoKw);
    stmt(p);
    p.expect(WhileKw);
    p.expect(LParen);
    expr::expr(p);
    p.expect(RParen);
    p.expect(Semi);
    m.complete(p, DoStmt);
}

pub(crate) fn switch_expr(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(SwitchKw);
    p.expect(LParen);
    expr::expr(p);
    p.expect(RParen);
    let sbm = p.start();
    p.expect(LBrace);
    while !p.at(RBrace) && p.kind() != Error {
        switch_label(p);
    }
    p.expect(RBrace);
    sbm.complete(p, SwitchBlock);
    m.complete(p, SwitchStmt);
}

pub(crate) fn switch_label(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    if p.eat(CaseKw) {
        expr::expr(p);
        if p.eat(Colon) {
            while !p.at_any(&[CaseKw, DefaultKw, RBrace]) && p.kind() != Error {
                stmt(p);
            }
        } else {
            p.expect(Arrow);
            let rm = p.start();
            if p.at(ThrowKw) {
                stmt(p);
            } else if p.at(LBrace) {
                block(p);
            } else {
                expr::expr(p);
                p.eat(Semi);
            }
            rm.complete(p, SwitchRule);
        }
    } else {
        p.expect(DefaultKw);
        if p.eat(Colon) {
            while !p.at_any(&[CaseKw, RBrace]) && p.kind() != Error {
                stmt(p);
            }
        } else {
            p.expect(Arrow);
            let rm = p.start();
            if p.at(ThrowKw) {
                stmt(p);
            } else if p.at(LBrace) {
                block(p);
            } else {
                expr::expr(p);
                p.eat(Semi);
            }
            rm.complete(p, SwitchRule);
        }
    }
    m.complete(p, SwitchLabel);
}

pub(crate) fn try_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(TryKw);
    if p.at(LParen) {
        let twm = p.start();
        p.expect(LParen);
        while !p.at(RParen) && p.kind() != Error {
            let rm = p.start();
            top_level::modifier_list(p);
            ty::type_(p);
            p.expect(Ident);
            if p.eat(Eq) {
                expr::expr(p);
            }
            rm.complete(p, Resource);
            if !p.eat(Semi) {
                break;
            }
        }
        p.expect(RParen);
        twm.complete(p, TryWithResources);
    }
    block(p);
    while p.at(CatchKw) {
        catch_clause(p);
    }
    if p.at(FinallyKw) {
        let fm = p.start();
        p.expect(FinallyKw);
        block(p);
        fm.complete(p, FinallyClause);
    }
    m.complete(p, TryStmt);
}

pub(crate) fn catch_clause(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(CatchKw);
    p.expect(LParen);
    top_level::modifier_list(p);
    ty::type_(p);
    p.expect(Ident);
    p.expect(RParen);
    block(p);
    m.complete(p, CatchClause);
}

pub(crate) fn return_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(ReturnKw);
    if !p.at(Semi) {
        expr::expr(p);
    }
    p.expect(Semi);
    m.complete(p, ReturnStmt);
}

pub(crate) fn throw_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(ThrowKw);
    expr::expr(p);
    p.expect(Semi);
    m.complete(p, ThrowStmt);
}

pub(crate) fn break_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(BreakKw);
    if p.at(Ident) {
        p.bump();
    }
    p.expect(Semi);
    m.complete(p, BreakStmt);
}

pub(crate) fn continue_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(ContinueKw);
    if p.at(Ident) {
        p.bump();
    }
    p.expect(Semi);
    m.complete(p, ContinueStmt);
}

pub(crate) fn synchronized_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(SynchronizedKw);
    p.expect(LParen);
    expr::expr(p);
    p.expect(RParen);
    block(p);
    m.complete(p, SynchronizedStmt);
}

pub(crate) fn assert_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(AssertKw);
    expr::expr(p);
    if p.eat(Colon) {
        expr::expr(p);
    }
    p.expect(Semi);
    m.complete(p, AssertStmt);
}

pub(crate) fn yield_stmt(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(YieldKw);
    expr::expr(p);
    p.expect(Semi);
    m.complete(p, YieldStmt);
}

pub(crate) fn is_local_var_decl(p: &Parser) -> bool {
    use JavaSyntaxKind::*;
    let primitives = [
        IntKw, LongKw, ShortKw, ByteKw, CharKw, FloatKw, DoubleKw, BooleanKw, VoidKw, VarKw,
    ];
    if p.at_any(&primitives) {
        return true;
    }
    if !p.at(Ident) {
        return false;
    }
    let mut la = p.lookahead();
    la.skip_type();
    la.skip_array_dims();
    la.at(Ident)
}

pub(crate) fn local_var_decl(p: &mut Parser) {
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
    m.complete(p, LocalVarDecl);
}

pub(crate) fn expr_or_local_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    if is_local_var_decl(p) {
        local_var_decl(p);
    } else {
        let m = p.start();
        expr::expr(p);
        p.expect(Semi);
        m.complete(p, ExprStmt);
    }
}
