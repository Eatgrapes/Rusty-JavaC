mod assign;
mod calls;
mod convert;
mod literals;
mod ops;
pub(crate) mod switch;
mod types;
mod values;

use crate::codegen::CodegenCtx;
use javac_classfile::MethodWriter;
use javac_hir::hir::*;
use javac_ty::Ty;
use rust_asm::opcodes;

pub(crate) use convert::{coerce, pop_ty, push_default_value};
pub(crate) use types::expr_ty;

pub fn gen_expr(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, expr_id: ExprId) {
    match &body.exprs[expr_id] {
        Expr::IntLiteral(value) => literals::emit_int(mw, *value),
        Expr::LongLiteral(value) => literals::emit_long(mw, *value),
        Expr::FloatLiteral(value) => literals::emit_float(mw, *value),
        Expr::DoubleLiteral(value) => literals::emit_double(mw, *value),
        Expr::BoolLiteral(value) => literals::emit_bool(mw, *value),
        Expr::NullLiteral => mw.visit_insn(opcodes::ACONST_NULL),
        Expr::StringLiteral(value) => mw.visit_ldc_insn_string(value),
        Expr::CharLiteral(value) => literals::emit_int(mw, *value as i64),
        Expr::This => mw.visit_var_insn(opcodes::ALOAD, 0),
        Expr::Ident(name) => values::emit_name(mw, ctx, *name),
        Expr::FieldAccess { target, field } => {
            if !calls::emit_field_access(mw, ctx, body, *target, *field) {
                discard_expr(mw, ctx, body, *target);
                push_default_value(mw, &expr_ty(ctx, body, expr_id));
            }
        }
        Expr::MethodCall {
            target,
            method,
            args,
        } => {
            if !calls::emit_method_call(mw, ctx, body, *target, *method, args) {
                if let Some(target) = target {
                    discard_expr(mw, ctx, body, *target);
                }
                for arg in args {
                    discard_expr(mw, ctx, body, *arg);
                }
                push_default_value(mw, &expr_ty(ctx, body, expr_id));
            }
        }
        Expr::Binary { op, left, right } => {
            ops::emit_binary(mw, ctx, body, op.clone(), *left, *right);
        }
        Expr::Switch {
            selector, cases, ..
        } => switch::emit_switch_expr(mw, ctx, body, *selector, cases),
        Expr::Unary { op, operand } => ops::emit_unary(mw, ctx, body, op, *operand),
        Expr::NewObject { class, args } => {
            let owner = class.internal_name();
            mw.visit_type_insn(opcodes::NEW, &owner);
            mw.visit_insn(opcodes::DUP);
            let mut descriptor = String::from("(");
            for arg in args {
                gen_expr(mw, ctx, body, *arg);
                descriptor.push_str(&expr_ty(ctx, body, *arg).erasure().descriptor());
            }
            descriptor.push_str(")V");
            mw.visit_method_insn(opcodes::INVOKESPECIAL, &owner, "<init>", &descriptor, false);
        }
        Expr::Parens(inner) => gen_expr(mw, ctx, body, *inner),
        Expr::Cast { ty, expr } => {
            gen_expr(mw, ctx, body, *expr);
            coerce(mw, &expr_ty(ctx, body, *expr), ty);
        }
        Expr::Assign { target, op, value } => {
            assign::emit_assign(mw, ctx, body, *target, op, *value)
        }
        Expr::PostInc(target) => assign::emit_post_inc_dec(mw, ctx, body, *target, 1),
        Expr::PostDec(target) => assign::emit_post_inc_dec(mw, ctx, body, *target, -1),
        Expr::Instanceof { expr, ty, .. } => {
            gen_expr(mw, ctx, body, *expr);
            mw.visit_type_insn(opcodes::INSTANCEOF, &ty.internal_name());
        }
        _ => push_default_value(mw, &expr_ty(ctx, body, expr_id)),
    }
}

pub(crate) fn discard_expr(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    expr_id: ExprId,
) {
    gen_expr(mw, ctx, body, expr_id);
    pop_ty(mw, &expr_ty(ctx, body, expr_id));
}

pub(crate) fn is_string(ty: &Ty) -> bool {
    matches!(ty.erasure(), Ty::Class(name) if name.as_str() == "java/lang/String")
}
