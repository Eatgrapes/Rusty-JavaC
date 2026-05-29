#[path = "expr_gen/arrays.rs"]
mod arrays;
#[path = "expr_gen/assign.rs"]
mod assign;
#[path = "expr_gen/branch.rs"]
pub(crate) mod branch;
#[path = "expr_gen/calls.rs"]
mod calls;
#[path = "expr_gen/convert.rs"]
mod convert;
#[path = "expr_gen/literals.rs"]
mod literals;
#[path = "expr_gen/ops.rs"]
mod ops;
#[path = "expr_gen/switch.rs"]
pub(crate) mod switch;
#[path = "expr_gen/types.rs"]
mod types;
#[path = "expr_gen/values.rs"]
mod values;

use crate::codegen::CodegenCtx;
use javac_classfile::MethodWriter;
use javac_hir::hir::*;
use javac_ty::Ty;
use rust_asm::opcodes;

pub(crate) use arrays::array_load_opcode;
pub(crate) use convert::{cast, coerce, pop_ty, push_default_value};
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
        Expr::This | Expr::Super => mw.visit_var_insn(opcodes::ALOAD, 0),
        Expr::Ident(name) => values::emit_name(mw, ctx, *name),
        Expr::ClassName(_) => push_default_value(mw, &Ty::object()),
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
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => emit_ternary(mw, ctx, body, expr_id, *condition, *then_expr, *else_expr),
        Expr::Unary { op, operand } => ops::emit_unary(mw, ctx, body, op, *operand),
        Expr::NewObject {
            class,
            args,
            anonymous,
        } => {
            let owner = anonymous
                .as_ref()
                .map(|info| info.class_name.to_string())
                .unwrap_or_else(|| class.internal_name());
            let arg_types = args
                .iter()
                .map(|arg| expr_ty(ctx, body, *arg))
                .collect::<Vec<_>>();
            let constructor_params = anonymous
                .as_ref()
                .map(|info| info.constructor_params.clone())
                .or_else(|| {
                    ctx.catalog
                        .resolve_constructor(&class.internal_name(), &arg_types)
                        .map(|ctor| ctor.params)
                })
                .unwrap_or_else(|| arg_types.clone());
            mw.visit_type_insn(opcodes::NEW, &owner);
            mw.visit_insn(opcodes::DUP);
            let mut descriptor = String::from("(");
            if anonymous
                .as_ref()
                .is_some_and(|info| info.captures_enclosing_this)
            {
                mw.visit_var_insn(opcodes::ALOAD, 0);
                descriptor.push_str(&Ty::Class(ctx.class_name).descriptor());
            }
            for (index, arg) in args.iter().enumerate() {
                gen_expr(mw, ctx, body, *arg);
                let arg_ty = &arg_types[index];
                let param_ty = constructor_params
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| arg_ty.clone());
                coerce(mw, arg_ty, &param_ty);
                descriptor.push_str(&param_ty.erasure().descriptor());
            }
            descriptor.push_str(")V");
            mw.visit_method_insn(opcodes::INVOKESPECIAL, &owner, "<init>", &descriptor, false);
        }
        Expr::Parens(inner) => gen_expr(mw, ctx, body, *inner),
        Expr::Cast { ty, expr } => {
            gen_expr(mw, ctx, body, *expr);
            cast(mw, &expr_ty(ctx, body, *expr), ty);
        }
        Expr::NewArray {
            element_type,
            dimensions,
            initializer,
        } => arrays::emit_new_array(
            mw,
            ctx,
            body,
            element_type,
            dimensions,
            initializer.as_ref(),
        ),
        Expr::ArrayAccess { array, index } => {
            arrays::emit_array_access(mw, ctx, body, *array, *index)
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
        Expr::Lambda { .. } => crate::lambda::emit_invokedynamic(mw, ctx, expr_id),
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

pub(crate) fn gen_expr_for_effect(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    expr_id: ExprId,
) {
    match &body.exprs[expr_id] {
        Expr::Assign { target, op, value }
            if assign::emit_assign_for_effect(mw, ctx, body, *target, op, *value) => {}
        Expr::PostInc(target)
        | Expr::Unary {
            op: UnaryOp::PreInc,
            operand: target,
        } if assign::emit_inc_dec_for_effect(mw, ctx, body, *target, 1) => {}
        Expr::PostDec(target)
        | Expr::Unary {
            op: UnaryOp::PreDec,
            operand: target,
        } if assign::emit_inc_dec_for_effect(mw, ctx, body, *target, -1) => {}
        _ => discard_expr(mw, ctx, body, expr_id),
    }
}

pub(crate) fn is_string(ty: &Ty) -> bool {
    ty.is_string()
}

fn emit_ternary(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    expr_id: ExprId,
    condition: ExprId,
    then_expr: ExprId,
    else_expr: ExprId,
) {
    let else_label = javac_classfile::Label::new();
    let end_label = javac_classfile::Label::new();
    let result_ty = expr_ty(ctx, body, expr_id);

    gen_expr(mw, ctx, body, condition);
    mw.visit_jump_insn(opcodes::IFEQ, else_label);
    gen_expr(mw, ctx, body, then_expr);
    coerce(mw, &expr_ty(ctx, body, then_expr), &result_ty);
    mw.visit_jump_insn(opcodes::GOTO, end_label);
    mw.visit_label(else_label);
    gen_expr(mw, ctx, body, else_expr);
    coerce(mw, &expr_ty(ctx, body, else_expr), &result_ty);
    mw.visit_label(end_label);
}
