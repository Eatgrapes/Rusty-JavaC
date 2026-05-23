use crate::lowering::{LowerError, LowerResult};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode, JavaSyntaxToken};
use javac_ty::Ty;
use ustr::Ustr;

pub(super) fn lower_type(node: &JavaSyntaxNode) -> LowerResult<Ty> {
    let mut base = lower_base_type(node)?;
    for _ in 0..array_dimensions(node) {
        base = Ty::Array(Box::new(base));
    }
    Ok(base)
}

pub(super) fn is_var_type(node: &JavaSyntaxNode) -> bool {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == JavaSyntaxKind::VarKw)
}

pub(super) fn is_string_ty(ty: &Ty) -> bool {
    matches!(ty, Ty::Class(name) if name.as_str() == "java/lang/String")
}

pub(super) fn class_type_from_name(name: &str) -> Ty {
    Ty::Class(Ustr::from(&class_internal_name(name)))
}

fn lower_base_type(node: &JavaSyntaxNode) -> LowerResult<Ty> {
    let Some(token) = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(is_type_token)
    else {
        return Err(LowerError::MissingType);
    };

    let ty = match token.kind() {
        JavaSyntaxKind::VoidKw => Ty::Void,
        JavaSyntaxKind::BooleanKw => Ty::Boolean,
        JavaSyntaxKind::ByteKw => Ty::Byte,
        JavaSyntaxKind::CharKw => Ty::Char,
        JavaSyntaxKind::ShortKw => Ty::Short,
        JavaSyntaxKind::IntKw => Ty::Int,
        JavaSyntaxKind::LongKw => Ty::Long,
        JavaSyntaxKind::FloatKw => Ty::Float,
        JavaSyntaxKind::DoubleKw => Ty::Double,
        JavaSyntaxKind::Ident => class_type_from_name(token.text()),
        JavaSyntaxKind::VarKw => Ty::Class(Ustr::from("java/lang/Object")),
        _ => return Err(LowerError::MissingType),
    };
    Ok(ty)
}

fn is_type_token(token: &JavaSyntaxToken) -> bool {
    matches!(
        token.kind(),
        JavaSyntaxKind::VoidKw
            | JavaSyntaxKind::BooleanKw
            | JavaSyntaxKind::ByteKw
            | JavaSyntaxKind::CharKw
            | JavaSyntaxKind::ShortKw
            | JavaSyntaxKind::IntKw
            | JavaSyntaxKind::LongKw
            | JavaSyntaxKind::FloatKw
            | JavaSyntaxKind::DoubleKw
            | JavaSyntaxKind::Ident
            | JavaSyntaxKind::VarKw
    )
}

fn class_internal_name(name: &str) -> String {
    match name {
        "String" => "java/lang/String".to_string(),
        "Object" => "java/lang/Object".to_string(),
        "Integer" => "java/lang/Integer".to_string(),
        _ => name.replace('.', "/"),
    }
}

fn array_dimensions(node: &JavaSyntaxNode) -> usize {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == JavaSyntaxKind::LBrack)
        .count()
}
