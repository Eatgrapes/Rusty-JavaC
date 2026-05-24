use crate::codegen::CodegenCtx;
use crate::expr_gen::{calls, is_string, values};
use javac_hir::hir::*;
use javac_ty::Ty;

pub(crate) fn expr_ty(ctx: &CodegenCtx, body: &Body, expr_id: ExprId) -> Ty {
    match &body.exprs[expr_id] {
        Expr::Ident(name) => ctx
            .local_ty(*name)
            .or_else(|| ctx.field_ty(*name))
            .unwrap_or(Ty::Int),
        Expr::FieldAccess { target, field } => {
            if let Some(field_ref) = calls::static_field_ref(ctx, body, *target, *field) {
                field_ref.ty
            } else if values::is_current_instance(body, *target) {
                ctx.field_ty(*field).unwrap_or(Ty::Int)
            } else {
                intrinsic_expr_ty(ctx, body, expr_id)
            }
        }
        Expr::MethodCall {
            target,
            method,
            args,
        } => calls::method_return_ty(ctx, body, *target, *method, args)
            .unwrap_or_else(|| intrinsic_expr_ty(ctx, body, expr_id)),
        Expr::Binary { op, left, right } => match op {
            BinaryOp::AndAnd
            | BinaryOp::OrOr
            | BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::Le
            | BinaryOp::Ge => Ty::Boolean,
            BinaryOp::Add
                if is_string(&expr_ty(ctx, body, *left))
                    || is_string(&expr_ty(ctx, body, *right)) =>
            {
                Ty::string()
            }
            _ => expr_ty(ctx, body, *left),
        },
        Expr::Unary { op, operand } => match op {
            UnaryOp::Not => Ty::Boolean,
            _ => expr_ty(ctx, body, *operand),
        },
        Expr::NewArray { element_type, .. } => Ty::Array(Box::new(element_type.clone())),
        Expr::ArrayAccess { array, .. } => match expr_ty(ctx, body, *array) {
            Ty::Array(element) => *element,
            _ => Ty::Int,
        },
        Expr::Ternary { then_expr, .. } => expr_ty(ctx, body, *then_expr),
        Expr::Assign { target, .. } => expr_ty(ctx, body, *target),
        Expr::Parens(inner) => expr_ty(ctx, body, *inner),
        Expr::Cast { ty, .. } => ty.clone(),
        Expr::Instanceof { .. } => Ty::Boolean,
        Expr::Switch { ty, .. } => ty.clone(),
        _ => intrinsic_expr_ty(ctx, body, expr_id),
    }
}

fn intrinsic_expr_ty(ctx: &CodegenCtx, body: &Body, expr_id: ExprId) -> Ty {
    match &body.exprs[expr_id] {
        Expr::IntLiteral(_) => Ty::Int,
        Expr::LongLiteral(_) => Ty::Long,
        Expr::FloatLiteral(_) => Ty::Float,
        Expr::DoubleLiteral(_) => Ty::Double,
        Expr::BoolLiteral(_) => Ty::Boolean,
        Expr::CharLiteral(_) => Ty::Char,
        Expr::StringLiteral(_) => Ty::string(),
        Expr::NullLiteral | Expr::This | Expr::Super => Ty::object(),
        Expr::ClassName(name) => Ty::Class(*name),
        Expr::NewObject { class, .. } => class.clone(),
        Expr::Lambda { .. } | Expr::MethodRef { .. } => Ty::object(),
        Expr::PostInc(inner) | Expr::PostDec(inner) | Expr::Parens(inner) => {
            expr_ty(ctx, body, *inner)
        }
        Expr::Assign { value, .. } => expr_ty(ctx, body, *value),
        _ => Ty::Int,
    }
}
