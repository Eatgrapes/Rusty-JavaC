use crate::codegen::CodegenCtx;
use crate::expr_gen::convert::{coerce, dup_ty, push_default_value};
use crate::expr_gen::{expr_ty, gen_expr};
use crate::local_var::{load_opcode, store_opcode};
use javac_classfile::MethodWriter;
use javac_hir::hir::*;
use javac_ty::Ty;
use rust_asm::opcodes;

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

    if let Expr::FieldAccess { target, field } = body.exprs[target].clone() {
        if matches!(op, AssignOp::Plain) && super::values::is_current_instance(body, target) {
            emit_instance_field_assign(mw, ctx, body, field, value);
            return;
        }
    }

    if let Expr::ArrayAccess { array, index } = body.exprs[target].clone() {
        if matches!(op, AssignOp::Plain) {
            emit_array_assign(mw, ctx, body, array, index, value);
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

fn emit_instance_field_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    field: ustr::Ustr,
    value: ExprId,
) {
    let ty = ctx
        .field_ty(field)
        .unwrap_or_else(|| expr_ty(ctx, body, value));
    let slot = ctx.alloc_temp(&ty);

    gen_expr(mw, ctx, body, value);
    coerce(mw, &expr_ty(ctx, body, value), &ty);
    mw.visit_var_insn(store_opcode(&ty), slot);

    mw.visit_var_insn(opcodes::ALOAD, 0);
    mw.visit_var_insn(load_opcode(&ty), slot);
    mw.visit_field_insn(
        opcodes::PUTFIELD,
        ctx.class_name.as_str(),
        field.as_str(),
        &ty.descriptor(),
    );
    mw.visit_var_insn(load_opcode(&ty), slot);
}

fn emit_array_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    array: ExprId,
    index: ExprId,
    value: ExprId,
) {
    let element_ty = super::arrays::array_element_type(ctx, body, array);
    let slot = ctx.alloc_temp(&element_ty);

    gen_expr(mw, ctx, body, value);
    coerce(mw, &expr_ty(ctx, body, value), &element_ty);
    mw.visit_var_insn(store_opcode(&element_ty), slot);

    gen_expr(mw, ctx, body, array);
    gen_expr(mw, ctx, body, index);
    coerce(mw, &expr_ty(ctx, body, index), &Ty::Int);
    mw.visit_var_insn(load_opcode(&element_ty), slot);
    mw.visit_insn(super::arrays::array_store_opcode(&element_ty));
    mw.visit_var_insn(load_opcode(&element_ty), slot);
}
