use crate::codegen::CodegenCtx;
use crate::expr_gen::convert::push_default_value;
use javac_call_resolver::system;
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
        mw.visit_var_insn(opcodes::ALOAD, 0);
        mw.visit_field_insn(
            opcodes::GETFIELD,
            ctx.class_name.as_str(),
            name.as_str(),
            &ty.descriptor(),
        );
    } else {
        push_default_value(mw, &Ty::Int);
    }
}

pub(super) fn is_current_instance(body: &Body, expr_id: ExprId) -> bool {
    matches!(body.exprs[expr_id], Expr::This)
}

pub(super) fn static_class_name(body: &Body, expr_id: ExprId) -> Option<&'static str> {
    match &body.exprs[expr_id] {
        Expr::Ident(name) => system::class_name(name.as_str()),
        _ => None,
    }
}
