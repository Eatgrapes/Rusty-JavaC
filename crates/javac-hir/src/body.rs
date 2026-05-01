use crate::hir::*;
use javac_ty::Ty;

impl Expr {
    pub fn ty(&self) -> Ty {
        match self {
            Expr::IntLiteral(_) => Ty::Int,
            Expr::LongLiteral(_) => Ty::Long,
            Expr::FloatLiteral(_) => Ty::Float,
            Expr::DoubleLiteral(_) => Ty::Double,
            Expr::BoolLiteral(_) => Ty::Boolean,
            Expr::CharLiteral(_) => Ty::Char,
            Expr::StringLiteral(_) => Ty::Class("java/lang/String".to_string()),
            Expr::NullLiteral => Ty::Class("java/lang/Object".to_string()),
            Expr::This => Ty::Class("java/lang/Object".to_string()),
            Expr::Super => Ty::Class("java/lang/Object".to_string()),
            Expr::Ident(_) => Ty::Int,
            Expr::FieldAccess { .. } => Ty::Int,
            Expr::MethodCall { .. } => Ty::Void,
            Expr::NewObject { class, .. } => class.clone(),
            Expr::NewArray { element_type, .. } => Ty::Array(Box::new(element_type.clone())),
            Expr::ArrayAccess { .. } => Ty::Int,
            Expr::Unary { op, operand } => match op {
                UnaryOp::Not => Ty::Boolean,
                _ => operand.ty(),
            },
            Expr::Binary { op, left, .. } => match op {
                BinaryOp::AndAnd | BinaryOp::OrOr => Ty::Boolean,
                BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => Ty::Boolean,
                _ => left.ty(),
            },
            Expr::Ternary { then_expr, .. } => then_expr.ty(),
            Expr::Cast { ty, .. } => ty.clone(),
            Expr::Instanceof { .. } => Ty::Boolean,
            Expr::Assign { value, .. } => value.ty(),
            Expr::PostInc(inner) | Expr::PostDec(inner) => inner.ty(),
            Expr::Lambda { .. } => Ty::Class("java/lang/Object".to_string()),
            Expr::MethodRef { .. } => Ty::Class("java/lang/Object".to_string()),
            Expr::Parens(inner) => inner.ty(),
        }
    }
}