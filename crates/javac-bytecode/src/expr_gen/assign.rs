use crate::codegen::CodegenCtx;
use crate::expr_gen::convert::{coerce, dup_ty, push_default_value};
use crate::expr_gen::{expr_ty, gen_expr};
use crate::local_var::{load_opcode, store_opcode};
use javac_classfile::MethodWriter;
use javac_hir::hir::*;
use javac_ty::Ty;

pub(super) fn emit_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    op: &AssignOp,
    value: ExprId,
) {
    if let Expr::Ident(name) = &body.exprs[target] {
        if let Some(slot) = ctx.get_local(*name) {
            emit_local_assign(mw, ctx, body, *name, slot, op, value);
            return;
        }
    }

    gen_expr(mw, ctx, body, value);
}

pub(super) fn emit_pre_inc_dec(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    amount: i16,
) {
    if let Expr::Ident(name) = &body.exprs[target] {
        if let Some(slot) = ctx.get_local(*name) {
            let ty = ctx.local_ty(*name).unwrap_or(Ty::Int);
            mw.visit_iinc_insn(slot, amount);
            mw.visit_var_insn(load_opcode(&ty), slot);
            return;
        }
    }

    push_default_value(mw, &expr_ty(ctx, body, target));
}

pub(super) fn emit_post_inc_dec(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    amount: i16,
) {
    if let Expr::Ident(name) = &body.exprs[target] {
        if let Some(slot) = ctx.get_local(*name) {
            let ty = ctx.local_ty(*name).unwrap_or(Ty::Int);
            mw.visit_var_insn(load_opcode(&ty), slot);
            mw.visit_iinc_insn(slot, amount);
            return;
        }
    }

    push_default_value(mw, &expr_ty(ctx, body, target));
}

fn emit_local_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    name: ustr::Ustr,
    slot: u16,
    op: &AssignOp,
    value: ExprId,
) {
    let ty = ctx.local_ty(name).unwrap_or(Ty::Int);

    if !matches!(op, AssignOp::Plain) {
        mw.visit_var_insn(load_opcode(&ty), slot);
    }

    gen_expr(mw, ctx, body, value);
    coerce(mw, &expr_ty(ctx, body, value), &ty);

    if !matches!(op, AssignOp::Plain) {
        super::ops::emit_assign_op(mw, op, &ty);
    }

    dup_ty(mw, &ty);
    mw.visit_var_insn(store_opcode(&ty), slot);
}
