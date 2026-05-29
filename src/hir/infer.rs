use crate::hir::{BinaryOp, Body, Expr, ExprId, UnaryOp};
use crate::ty::Ty;
use crate::ty::check::numeric_promotion;
use ustr::Ustr;

pub trait TypeEnvironment {
    fn local_ty(&self, name: Ustr) -> Option<Ty>;

    fn field_ty(&self, name: Ustr) -> Option<Ty>;

    fn resolve_static_field(&self, owner: &str, name: &str) -> Option<Ty>;

    fn resolve_instance_method(&self, receiver: &Ty, name: &str, args: &[Ty]) -> Option<Ty>;

    fn resolve_static_method(&self, _owner: &str, _name: &str, _args: &[Ty]) -> Option<Ty> {
        None
    }

    fn resolve_current_method(&self, _name: Ustr, _args: &[Ty]) -> Option<Ty> {
        None
    }

    fn this_ty(&self) -> Ty {
        Ty::object()
    }

    fn super_ty(&self) -> Ty {
        Ty::object()
    }
}

pub fn expr_ty(env: &impl TypeEnvironment, body: &Body, expr_id: ExprId) -> Ty {
    match &body.exprs[expr_id] {
        Expr::Ident(name) => env
            .local_ty(*name)
            .or_else(|| env.field_ty(*name))
            .unwrap_or(Ty::Int),
        Expr::This => env.this_ty(),
        Expr::Super => env.super_ty(),
        Expr::ClassName(name) => Ty::Class(*name),
        Expr::FieldAccess { target, field } => {
            if field.as_str() == "length"
                && matches!(expr_ty(env, body, *target).erasure(), Ty::Array(_))
            {
                return Ty::Int;
            }
            if let Some(owner) = static_class_name(body, *target)
                && let Some(ty) = env.resolve_static_field(owner, field.as_str())
            {
                return ty;
            }
            if is_current_instance(body, *target)
                && let Some(ty) = env.field_ty(*field)
            {
                return ty;
            }
            intrinsic_expr_ty(env, body, expr_id)
        }
        Expr::MethodCall {
            target,
            method,
            args,
        } => {
            let arg_types = args
                .iter()
                .map(|arg| expr_ty(env, body, *arg))
                .collect::<Vec<_>>();
            if let Some(target) = target {
                if let Some(owner) = static_class_name(body, *target)
                    && let Some(return_ty) =
                        env.resolve_static_method(owner, method.as_str(), &arg_types)
                {
                    return return_ty;
                }
                let receiver = expr_ty(env, body, *target);
                if let Some(return_ty) =
                    env.resolve_instance_method(&receiver, method.as_str(), &arg_types)
                {
                    return return_ty;
                }
            } else if let Some(return_ty) = env.resolve_current_method(*method, &arg_types) {
                return return_ty;
            }
            intrinsic_expr_ty(env, body, expr_id)
        }
        Expr::Binary { op, left, right } => binary_expr_ty(env, body, op, *left, *right),
        Expr::Unary { op, operand } => match op {
            UnaryOp::Not => Ty::Boolean,
            _ => expr_ty(env, body, *operand),
        },
        Expr::NewArray { element_type, .. } => Ty::Array(Box::new(element_type.clone())),
        Expr::ArrayAccess { array, .. } => match expr_ty(env, body, *array) {
            Ty::Array(element) => *element,
            _ => Ty::Int,
        },
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => common_conditional_ty(env, body, *then_expr, *else_expr),
        Expr::Assign { target, .. } => expr_ty(env, body, *target),
        Expr::Parens(inner) => expr_ty(env, body, *inner),
        Expr::Cast { ty, .. } => ty.clone(),
        Expr::Instanceof { .. } => Ty::Boolean,
        Expr::Switch { ty, .. } => ty.clone(),
        Expr::Lambda {
            target_ty: Some(ty),
            ..
        } => ty.clone(),
        _ => intrinsic_expr_ty(env, body, expr_id),
    }
}

pub fn lambda_body_ty(
    env: &impl TypeEnvironment,
    body: &Body,
    lambda_body: &crate::hir::LambdaBody,
) -> Ty {
    match lambda_body {
        crate::hir::LambdaBody::Expr(expr) => expr_ty(env, body, *expr),
        crate::hir::LambdaBody::Block(_) => Ty::Void,
    }
}

fn intrinsic_expr_ty(env: &impl TypeEnvironment, body: &Body, expr_id: ExprId) -> Ty {
    match &body.exprs[expr_id] {
        Expr::IntLiteral(_) => Ty::Int,
        Expr::LongLiteral(_) => Ty::Long,
        Expr::FloatLiteral(_) => Ty::Float,
        Expr::DoubleLiteral(_) => Ty::Double,
        Expr::BoolLiteral(_) => Ty::Boolean,
        Expr::CharLiteral(_) => Ty::Char,
        Expr::StringLiteral(_) => Ty::string(),
        Expr::NullLiteral => Ty::Wildcard(None),
        Expr::This => env.this_ty(),
        Expr::Super => env.super_ty(),
        Expr::ClassName(name) => Ty::Class(*name),
        Expr::NewObject {
            class, anonymous, ..
        } => anonymous
            .as_ref()
            .map(|info| Ty::Class(info.class_name))
            .unwrap_or_else(|| class.clone()),
        Expr::PostInc(inner) | Expr::PostDec(inner) | Expr::Parens(inner) => {
            expr_ty(env, body, *inner)
        }
        Expr::Assign { value, .. } => expr_ty(env, body, *value),
        Expr::Lambda {
            target_ty: Some(ty),
            ..
        } => ty.clone(),
        Expr::Lambda { .. } | Expr::MethodRef { .. } => Ty::object(),
        _ => Ty::Int,
    }
}

fn binary_expr_ty(
    env: &impl TypeEnvironment,
    body: &Body,
    op: &BinaryOp,
    left: ExprId,
    right: ExprId,
) -> Ty {
    let left_ty = expr_ty(env, body, left);
    let right_ty = expr_ty(env, body, right);
    match op {
        BinaryOp::AndAnd
        | BinaryOp::OrOr
        | BinaryOp::Eq
        | BinaryOp::Ne
        | BinaryOp::Lt
        | BinaryOp::Gt
        | BinaryOp::Le
        | BinaryOp::Ge => Ty::Boolean,
        BinaryOp::Add if left_ty.is_string() || right_ty.is_string() => Ty::string(),
        BinaryOp::Shl | BinaryOp::Shr | BinaryOp::Ushr => unary_numeric_promotion(&left_ty),
        _ => numeric_promotion(&left_ty, &right_ty).unwrap_or(left_ty),
    }
}

fn common_conditional_ty(
    env: &impl TypeEnvironment,
    body: &Body,
    then_expr: ExprId,
    else_expr: ExprId,
) -> Ty {
    let then_ty = expr_ty(env, body, then_expr);
    let else_ty = expr_ty(env, body, else_expr);
    if then_ty == else_ty {
        return then_ty;
    }
    if matches!(body.exprs[then_expr], Expr::NullLiteral) {
        return else_ty;
    }
    if matches!(body.exprs[else_expr], Expr::NullLiteral) {
        return then_ty;
    }
    numeric_promotion(&then_ty, &else_ty).unwrap_or(then_ty)
}

fn unary_numeric_promotion(ty: &Ty) -> Ty {
    match ty {
        Ty::Byte | Ty::Short | Ty::Char => Ty::Int,
        _ => ty.clone(),
    }
}

fn static_class_name(body: &Body, expr_id: ExprId) -> Option<&str> {
    match &body.exprs[expr_id] {
        Expr::ClassName(name) => Some(name.as_str()),
        _ => None,
    }
}

fn is_current_instance(body: &Body, expr_id: ExprId) -> bool {
    matches!(body.exprs[expr_id], Expr::This | Expr::Super)
}
