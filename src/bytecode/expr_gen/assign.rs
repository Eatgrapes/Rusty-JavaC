use crate::bytecode::codegen::CodegenCtx;
use crate::bytecode::expr_gen::convert::{coerce, dup_ty, push_default_value};
use crate::bytecode::expr_gen::{expr_ty, gen_expr};
use crate::bytecode::local_var::{load_opcode, store_opcode};
use crate::classfile::MethodWriter;
use crate::hir::*;
use crate::ty::Ty;
use rust_asm::opcodes;

#[derive(Clone, Copy)]
enum AssignMode {
    Value,
    Effect,
}

impl AssignMode {
    fn leaves_value(self) -> bool {
        matches!(self, Self::Value)
    }
}

#[derive(Clone, Copy)]
struct AssignRequest<'a> {
    body: &'a Body,
    op: &'a AssignOp,
    value: ExprId,
    mode: AssignMode,
}

pub(super) fn emit_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    op: &AssignOp,
    value: ExprId,
) {
    let request = AssignRequest {
        body,
        op,
        value,
        mode: AssignMode::Value,
    };
    if !emit_known_assign(mw, ctx, target, request) {
        gen_expr(mw, ctx, body, value);
    }
}

pub(super) fn emit_assign_for_effect(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    op: &AssignOp,
    value: ExprId,
) -> bool {
    emit_known_assign(
        mw,
        ctx,
        target,
        AssignRequest {
            body,
            op,
            value,
            mode: AssignMode::Effect,
        },
    )
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
        } else if ctx.field_is_static(*name) {
            emit_static_field_pre_inc_dec(mw, ctx, *name, amount);
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
        } else if ctx.field_is_static(*name) {
            emit_static_field_post_inc_dec(mw, ctx, *name, amount);
            return;
        }
    }

    push_default_value(mw, &expr_ty(ctx, body, target));
}

pub(super) fn emit_inc_dec_for_effect(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    amount: i16,
) -> bool {
    if let Expr::Ident(name) = &body.exprs[target]
        && let Some(slot) = ctx.get_local(*name)
    {
        mw.visit_iinc_insn(slot, amount);
        return true;
    }

    if let Expr::Ident(name) = &body.exprs[target]
        && ctx.field_is_static(*name)
    {
        emit_static_field_inc_dec_for_effect(mw, ctx, *name, amount);
        return true;
    }

    false
}

fn emit_local_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    request: AssignRequest<'_>,
    name: ustr::Ustr,
    slot: u16,
) {
    let ty = ctx.local_ty(name).unwrap_or(Ty::Int);

    if !matches!(request.op, AssignOp::Plain) {
        mw.visit_var_insn(load_opcode(&ty), slot);
    }

    gen_expr(mw, ctx, request.body, request.value);
    coerce(mw, &expr_ty(ctx, request.body, request.value), &ty);

    if !matches!(request.op, AssignOp::Plain) {
        super::ops::emit_assign_op(mw, request.op, &ty);
    }

    if request.mode.leaves_value() {
        dup_ty(mw, &ty);
    }
    mw.visit_var_insn(store_opcode(&ty), slot);
}

fn emit_known_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    target: ExprId,
    request: AssignRequest<'_>,
) -> bool {
    match request.body.exprs[target].clone() {
        Expr::Ident(name) => {
            if let Some(slot) = ctx.get_local(name) {
                emit_local_assign(mw, ctx, request, name, slot);
                true
            } else if ctx.field_is_static(name) {
                emit_static_field_assign(mw, ctx, request, name);
                true
            } else {
                false
            }
        }
        Expr::FieldAccess { target, field }
            if matches!(request.op, AssignOp::Plain)
                && super::values::is_current_instance(request.body, target) =>
        {
            emit_instance_field_assign(mw, ctx, request, field);
            true
        }
        Expr::ArrayAccess { array, index } => {
            emit_array_assign(mw, ctx, request, array, index);
            true
        }
        _ => false,
    }
}

fn emit_static_field_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    request: AssignRequest<'_>,
    name: ustr::Ustr,
) {
    let ty = ctx
        .field_ty(name)
        .unwrap_or_else(|| expr_ty(ctx, request.body, request.value));

    if !matches!(request.op, AssignOp::Plain) {
        mw.visit_field_insn(
            opcodes::GETSTATIC,
            ctx.class_name.as_str(),
            name.as_str(),
            &ty.descriptor(),
        );
    }

    gen_expr(mw, ctx, request.body, request.value);
    coerce(mw, &expr_ty(ctx, request.body, request.value), &ty);

    if !matches!(request.op, AssignOp::Plain) {
        super::ops::emit_assign_op(mw, request.op, &ty);
    }

    if request.mode.leaves_value() {
        dup_ty(mw, &ty);
    }
    mw.visit_field_insn(
        opcodes::PUTSTATIC,
        ctx.class_name.as_str(),
        name.as_str(),
        &ty.descriptor(),
    );
}

fn emit_static_field_pre_inc_dec(
    mw: &mut MethodWriter,
    ctx: &CodegenCtx,
    name: ustr::Ustr,
    amount: i16,
) {
    let ty = ctx.field_ty(name).unwrap_or(Ty::Int);
    mw.visit_field_insn(
        opcodes::GETSTATIC,
        ctx.class_name.as_str(),
        name.as_str(),
        &ty.descriptor(),
    );
    super::literals::emit_int(mw, amount as i64);
    super::ops::emit_assign_op(mw, &AssignOp::Add, &ty);
    dup_ty(mw, &ty);
    mw.visit_field_insn(
        opcodes::PUTSTATIC,
        ctx.class_name.as_str(),
        name.as_str(),
        &ty.descriptor(),
    );
}

fn emit_static_field_post_inc_dec(
    mw: &mut MethodWriter,
    ctx: &CodegenCtx,
    name: ustr::Ustr,
    amount: i16,
) {
    let ty = ctx.field_ty(name).unwrap_or(Ty::Int);
    mw.visit_field_insn(
        opcodes::GETSTATIC,
        ctx.class_name.as_str(),
        name.as_str(),
        &ty.descriptor(),
    );
    dup_ty(mw, &ty);
    super::literals::emit_int(mw, amount as i64);
    super::ops::emit_assign_op(mw, &AssignOp::Add, &ty);
    mw.visit_field_insn(
        opcodes::PUTSTATIC,
        ctx.class_name.as_str(),
        name.as_str(),
        &ty.descriptor(),
    );
}

fn emit_static_field_inc_dec_for_effect(
    mw: &mut MethodWriter,
    ctx: &CodegenCtx,
    name: ustr::Ustr,
    amount: i16,
) {
    let ty = ctx.field_ty(name).unwrap_or(Ty::Int);
    mw.visit_field_insn(
        opcodes::GETSTATIC,
        ctx.class_name.as_str(),
        name.as_str(),
        &ty.descriptor(),
    );
    super::literals::emit_int(mw, amount as i64);
    super::ops::emit_assign_op(mw, &AssignOp::Add, &ty);
    mw.visit_field_insn(
        opcodes::PUTSTATIC,
        ctx.class_name.as_str(),
        name.as_str(),
        &ty.descriptor(),
    );
}

fn emit_instance_field_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    request: AssignRequest<'_>,
    field: ustr::Ustr,
) {
    let ty = ctx
        .field_ty(field)
        .unwrap_or_else(|| expr_ty(ctx, request.body, request.value));

    if request.mode.leaves_value() {
        let slot = ctx.alloc_temp(&ty);
        gen_expr(mw, ctx, request.body, request.value);
        coerce(mw, &expr_ty(ctx, request.body, request.value), &ty);
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
    } else {
        mw.visit_var_insn(opcodes::ALOAD, 0);
        gen_expr(mw, ctx, request.body, request.value);
        coerce(mw, &expr_ty(ctx, request.body, request.value), &ty);
        mw.visit_field_insn(
            opcodes::PUTFIELD,
            ctx.class_name.as_str(),
            field.as_str(),
            &ty.descriptor(),
        );
    }
}

fn emit_array_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    request: AssignRequest<'_>,
    array: ExprId,
    index: ExprId,
) {
    let element_ty = super::arrays::array_element_type(ctx, request.body, array);

    gen_expr(mw, ctx, request.body, array);
    gen_expr(mw, ctx, request.body, index);
    coerce(mw, &expr_ty(ctx, request.body, index), &Ty::Int);

    if !matches!(request.op, AssignOp::Plain) {
        mw.visit_insn(opcodes::DUP2);
        mw.visit_insn(super::arrays::array_load_opcode(&element_ty));
    }

    gen_expr(mw, ctx, request.body, request.value);
    coerce(mw, &expr_ty(ctx, request.body, request.value), &element_ty);

    if !matches!(request.op, AssignOp::Plain) {
        super::ops::emit_assign_op(mw, request.op, &element_ty);
    }

    if request.mode.leaves_value() {
        dup_array_store_value(mw, &element_ty);
    }

    mw.visit_insn(super::arrays::array_store_opcode(&element_ty));
}

fn dup_array_store_value(mw: &mut MethodWriter, ty: &Ty) {
    mw.visit_insn(if ty.size() == 2 {
        opcodes::DUP2_X2
    } else {
        opcodes::DUP_X2
    });
}
