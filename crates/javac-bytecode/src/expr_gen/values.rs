use crate::codegen::CodegenCtx;
use crate::expr_gen::convert::push_default_value;
use javac_classfile::MethodWriter;
use javac_hir::hir::{Body, Expr, ExprId};
use javac_ty::Ty;
use rust_asm::opcodes;
use ustr::Ustr;

pub(super) fn emit_name(mw: &mut MethodWriter, ctx: &CodegenCtx, name: Ustr) {
    if let Some(slot) = ctx.get_local(name) {
        if let Some(ty) = ctx.local_ty(name) {
            mw.visit_var_insn(crate::local_var::load_opcode(&ty), slot);
        }
    } else if let Some(ty) = ctx.field_ty(name) {
        if ctx.field_is_static(name) {
            mw.visit_field_insn(
                opcodes::GETSTATIC,
                ctx.class_name.as_str(),
                name.as_str(),
                &ty.descriptor(),
            );
        } else if let Some(outer_field) = ctx.outer_fields.get(&name)
            && outer_field.access_flags & javac_classfile::ACC_STATIC != 0
        {
            let owner = ctx
                .outer_this
                .as_ref()
                .map(|outer_this| outer_this.ty.internal_name())
                .or_else(|| ctx.enclosing_static_owner.map(|owner| owner.to_string()));
            let Some(owner) = owner else {
                push_default_value(mw, &ty);
                return;
            };
            mw.visit_field_insn(
                opcodes::GETSTATIC,
                &owner,
                outer_field.name.as_str(),
                &outer_field.ty.descriptor(),
            );
        } else if let Some(outer_field) = ctx.outer_fields.get(&name)
            && let Some(outer_this) = &ctx.outer_this
        {
            mw.visit_var_insn(opcodes::ALOAD, 0);
            mw.visit_field_insn(
                opcodes::GETFIELD,
                ctx.class_name.as_str(),
                outer_this.field_name.as_str(),
                &outer_this.ty.descriptor(),
            );
            mw.visit_field_insn(
                opcodes::GETFIELD,
                &outer_this.ty.internal_name(),
                outer_field.name.as_str(),
                &outer_field.ty.descriptor(),
            );
        } else if let Some(owner) = ctx.enclosing_static_owner
            && let Some(field_ref) = ctx
                .catalog
                .resolve_static_field(owner.as_str(), name.as_str())
        {
            mw.visit_field_insn(
                opcodes::GETSTATIC,
                &field_ref.owner,
                &field_ref.name,
                &field_ref.descriptor,
            );
        } else {
            mw.visit_var_insn(opcodes::ALOAD, 0);
            mw.visit_field_insn(
                opcodes::GETFIELD,
                ctx.class_name.as_str(),
                name.as_str(),
                &ty.descriptor(),
            );
        }
    } else {
        push_default_value(mw, &Ty::Int);
    }
}

pub(super) fn is_current_instance(body: &Body, expr_id: ExprId) -> bool {
    matches!(body.exprs[expr_id], Expr::This)
}

pub(super) fn is_super(body: &Body, expr_id: ExprId) -> bool {
    matches!(body.exprs[expr_id], Expr::Super)
}

pub(super) fn static_class_name(body: &Body, expr_id: ExprId) -> Option<&str> {
    match &body.exprs[expr_id] {
        Expr::ClassName(name) => Some(name.as_str()),
        _ => None,
    }
}
