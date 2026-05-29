use crate::codegen::CodegenCtx;
use crate::expr_gen::{arrays, coerce, expr_ty, gen_expr, literals, values};
use javac_call_resolver::{FieldRef, MethodRef};
use javac_classfile::MethodWriter;
use javac_hir::hir::{Body, ExprId};
use javac_ty::Ty;
use rust_asm::opcodes;
use ustr::Ustr;

enum ReceiverStyle {
    Implicit,
    ExplicitThis,
}

pub(super) fn emit_field_access(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    field: Ustr,
) -> bool {
    if field.as_str() == "length" && matches!(expr_ty(ctx, body, target).erasure(), Ty::Array(_)) {
        gen_expr(mw, ctx, body, target);
        mw.visit_insn(opcodes::ARRAYLENGTH);
        return true;
    }

    if let Some(field_ref) = static_field_ref(ctx, body, target, field) {
        mw.visit_field_insn(
            opcodes::GETSTATIC,
            &field_ref.owner,
            &field_ref.name,
            &field_ref.descriptor,
        );
        return true;
    }

    if values::is_current_instance(body, target)
        && let Some(ty) = ctx.field_ty(field)
    {
        mw.visit_var_insn(opcodes::ALOAD, 0);
        mw.visit_field_insn(
            opcodes::GETFIELD,
            ctx.class_name.as_str(),
            field.as_str(),
            &ty.descriptor(),
        );
        return true;
    }

    false
}

pub(super) fn emit_method_call(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: Option<ExprId>,
    method: Ustr,
    args: &[ExprId],
) -> bool {
    if let Some(target) = target {
        let arg_types = arg_types(ctx, body, args);
        if let Some(method_ref) = ctx.catalog.resolve_instance_method(
            &expr_ty(ctx, body, target),
            method.as_str(),
            &arg_types,
        ) {
            gen_expr(mw, ctx, body, target);
            emit_call_args(mw, ctx, body, args, &method_ref);
            if values::is_super(body, target) {
                mw.visit_method_insn(
                    opcodes::INVOKESPECIAL,
                    &method_ref.owner,
                    &method_ref.name,
                    &method_ref.descriptor,
                    false,
                );
                return true;
            }
            mw.visit_method_insn(
                method_ref.opcode,
                &method_ref.owner,
                &method_ref.name,
                &method_ref.descriptor,
                method_ref.is_interface,
            );
            return true;
        }

        if values::is_current_instance(body, target) {
            return emit_current_class_call(
                mw,
                ctx,
                body,
                method,
                args,
                ReceiverStyle::ExplicitThis,
            );
        }
    } else if emit_current_class_call(mw, ctx, body, method, args, ReceiverStyle::Implicit)
        || emit_enclosing_static_call(mw, ctx, body, method, args)
    {
        return true;
    }

    false
}

fn emit_enclosing_static_call(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    method: Ustr,
    args: &[ExprId],
) -> bool {
    let Some(owner) = ctx.enclosing_static_owner else {
        return false;
    };
    let arg_types = arg_types(ctx, body, args);
    let Some(method_ref) =
        ctx.catalog
            .resolve_static_method(owner.as_str(), method.as_str(), &arg_types)
    else {
        return false;
    };

    for (arg, param_ty) in args.iter().zip(&method_ref.params) {
        emit_coerced_arg(mw, ctx, body, *arg, param_ty);
    }
    mw.visit_method_insn(
        opcodes::INVOKESTATIC,
        &method_ref.owner,
        &method_ref.name,
        &method_ref.descriptor,
        false,
    );
    true
}

pub(super) fn static_field_ref(
    ctx: &CodegenCtx,
    body: &Body,
    target: ExprId,
    field: Ustr,
) -> Option<FieldRef> {
    let owner = values::static_class_name(body, target)?;
    ctx.catalog.resolve_static_field(owner, field.as_str())
}

fn emit_current_class_call(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    method: Ustr,
    args: &[ExprId],
    receiver_style: ReceiverStyle,
) -> bool {
    let Some(sig) = ctx.method_sig(method) else {
        return emit_inherited_current_call(mw, ctx, body, method, args);
    };

    let is_static = sig.access_flags & javac_classfile::ACC_STATIC != 0;
    if !is_static {
        mw.visit_var_insn(opcodes::ALOAD, 0);
    } else if matches!(receiver_style, ReceiverStyle::ExplicitThis) {
        return false;
    }

    for (arg, param_ty) in args.iter().zip(&sig.params) {
        emit_coerced_arg(mw, ctx, body, *arg, param_ty);
    }

    mw.visit_method_insn(
        if is_static {
            opcodes::INVOKESTATIC
        } else {
            opcodes::INVOKEVIRTUAL
        },
        ctx.class_name.as_str(),
        method.as_str(),
        &sig.descriptor(),
        false,
    );
    true
}

fn emit_inherited_current_call(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    method: Ustr,
    args: &[ExprId],
) -> bool {
    let arg_types = arg_types(ctx, body, args);
    let Some(method_ref) = ctx.catalog.resolve_instance_method(
        &Ty::Class(ctx.super_name),
        method.as_str(),
        &arg_types,
    ) else {
        return false;
    };

    mw.visit_var_insn(opcodes::ALOAD, 0);
    emit_call_args(mw, ctx, body, args, &method_ref);
    mw.visit_method_insn(
        method_ref.opcode,
        &method_ref.owner,
        &method_ref.name,
        &method_ref.descriptor,
        method_ref.is_interface,
    );
    true
}

fn arg_types(ctx: &CodegenCtx, body: &Body, args: &[ExprId]) -> Vec<Ty> {
    args.iter().map(|arg| expr_ty(ctx, body, *arg)).collect()
}

fn emit_call_args(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    args: &[ExprId],
    method_ref: &MethodRef,
) {
    if method_ref.is_varargs
        && let Some(fixed_count) = method_ref.params.len().checked_sub(1)
        && args.len() >= fixed_count
        && should_expand_varargs(ctx, body, args, method_ref)
        && let Ty::Array(element_ty) = method_ref.params[fixed_count].erasure()
    {
        for (arg, param_ty) in args[..fixed_count]
            .iter()
            .zip(&method_ref.params[..fixed_count])
        {
            emit_coerced_arg(mw, ctx, body, *arg, param_ty);
        }
        emit_varargs_array(mw, ctx, body, &args[fixed_count..], &element_ty);
        return;
    }

    for (index, arg) in args.iter().copied().enumerate() {
        gen_expr(mw, ctx, body, arg);
        if let Some(param_ty) = method_ref.params.get(index) {
            coerce(mw, &expr_ty(ctx, body, arg), param_ty);
        }
    }
}

fn emit_coerced_arg(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    arg: ExprId,
    param_ty: &Ty,
) {
    gen_expr(mw, ctx, body, arg);
    coerce(mw, &expr_ty(ctx, body, arg), param_ty);
}

fn should_expand_varargs(
    ctx: &CodegenCtx,
    body: &Body,
    args: &[ExprId],
    method_ref: &MethodRef,
) -> bool {
    if args.len() != method_ref.params.len() {
        return true;
    }
    let Some(last_arg) = args.last().copied() else {
        return true;
    };
    expr_ty(ctx, body, last_arg).erasure() != method_ref.params.last().unwrap().erasure()
}

fn emit_varargs_array(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    args: &[ExprId],
    element_ty: &Ty,
) {
    literals::emit_int(mw, args.len() as i64);
    if element_ty.is_primitive() {
        mw.visit_new_array(arrays::primitive_array_type_code(element_ty));
    } else {
        mw.visit_type_insn(opcodes::ANEWARRAY, &element_ty.internal_name());
    }

    for (index, arg) in args.iter().copied().enumerate() {
        mw.visit_insn(opcodes::DUP);
        literals::emit_int(mw, index as i64);
        gen_expr(mw, ctx, body, arg);
        coerce(mw, &expr_ty(ctx, body, arg), element_ty);
        mw.visit_insn(arrays::array_store_opcode(element_ty));
    }
}
