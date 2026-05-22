use crate::codegen::CodegenCtx;
use crate::expr_gen::{expr_ty, gen_expr, values};
use javac_call_resolver::{self, FieldRef};
use javac_classfile::MethodWriter;
use javac_hir::hir::{Body, ExprId};
use javac_ty::Ty;
use rust_asm::opcodes;
use ustr::Ustr;

pub(super) fn emit_field_access(
    mw: &mut MethodWriter,
    ctx: &CodegenCtx,
    body: &Body,
    target: ExprId,
    field: Ustr,
) -> bool {
    if let Some(field_ref) = static_field_ref(body, target, field) {
        mw.visit_field_insn(
            opcodes::GETSTATIC,
            field_ref.owner,
            field_ref.name,
            field_ref.descriptor,
        );
        return true;
    }

    if values::is_current_instance(body, target) {
        if let Some(ty) = ctx.field_ty(field) {
            mw.visit_var_insn(opcodes::ALOAD, 0);
            mw.visit_field_insn(
                opcodes::GETFIELD,
                ctx.class_name.as_str(),
                field.as_str(),
                &ty.descriptor(),
            );
            return true;
        }
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
        if let Some(method_ref) = javac_call_resolver::resolve_instance_method(
            &expr_ty(ctx, body, target),
            method.as_str(),
            &arg_types,
        ) {
            gen_expr(mw, ctx, body, target);
            for arg in args {
                gen_expr(mw, ctx, body, *arg);
            }
            mw.visit_method_insn(
                method_ref.opcode,
                method_ref.owner,
                method_ref.name,
                &method_ref.descriptor,
                method_ref.is_interface,
            );
            return true;
        }

        if values::is_current_instance(body, target) {
            return emit_current_class_call(mw, ctx, body, method, args, true);
        }
    } else if emit_current_class_call(mw, ctx, body, method, args, false) {
        return true;
    }

    false
}

pub(super) fn method_return_ty(
    ctx: &CodegenCtx,
    body: &Body,
    target: Option<ExprId>,
    method: Ustr,
    args: &[ExprId],
) -> Option<Ty> {
    if let Some(target) = target {
        let receiver = expr_ty(ctx, body, target);
        let args = arg_types(ctx, body, args);
        if let Some(method_ref) =
            javac_call_resolver::resolve_instance_method(&receiver, method.as_str(), &args)
        {
            return Some(method_ref.return_ty);
        }
    }

    ctx.method_sig(method).map(|sig| sig.return_type)
}

pub(super) fn static_field_ref(body: &Body, target: ExprId, field: Ustr) -> Option<FieldRef> {
    let owner = values::static_class_name(body, target)?;
    javac_call_resolver::resolve_static_field(owner, field.as_str())
}

fn emit_current_class_call(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    method: Ustr,
    args: &[ExprId],
    explicit_target: bool,
) -> bool {
    let Some(sig) = ctx.method_sig(method) else {
        return false;
    };

    let is_static = sig.access_flags & javac_classfile::ACC_STATIC != 0;
    if !is_static {
        mw.visit_var_insn(opcodes::ALOAD, 0);
    } else if explicit_target {
        return false;
    }

    for arg in args {
        gen_expr(mw, ctx, body, *arg);
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

fn arg_types(ctx: &CodegenCtx, body: &Body, args: &[ExprId]) -> Vec<Ty> {
    args.iter().map(|arg| expr_ty(ctx, body, *arg)).collect()
}
