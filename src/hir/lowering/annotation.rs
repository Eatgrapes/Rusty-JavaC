use crate::ast::{AnnotationDecl, AstNode, JavaSyntaxKind, JavaSyntaxNode};
use crate::call_resolver::ClassCatalog;
use crate::hir::lowering::literal::{parse_int_literal, string_literal_value};
use crate::hir::lowering::member::{MemberOptions, lower_members};
use crate::hir::lowering::signature::lower_type_params;
use crate::hir::lowering::syntax::{ExprToken, qualified_name_text, tokens_in_first_parens};
use crate::hir::lowering::types::TypeResolver;
use crate::hir::lowering::unit::internal_class_name;
use crate::hir::lowering::{LowerError, LowerResult};
use crate::hir::*;
use crate::ty::Ty;
use ustr::Ustr;

pub(super) fn lower_annotation_decl(
    annotation: AnnotationDecl,
    access_flags: u16,
    modifiers: Option<&JavaSyntaxNode>,
    package: Option<&Package>,
    imports: &[Import],
    catalog: &ClassCatalog,
) -> LowerResult<TypeDecl> {
    let name = annotation.name().ok_or(LowerError::MissingClassName)?;
    let internal_name = internal_class_name(package, name.text());
    let resolver = TypeResolver::for_class(package, imports, &internal_name, catalog)?;
    let annotations = lower_annotation_uses(modifiers, package, &resolver)?;
    let type_params = lower_type_params(annotation.syntax(), &resolver)?;
    let members = annotation
        .body()
        .map(|body| {
            lower_members(
                body.syntax(),
                &type_params,
                &resolver,
                Some(Ustr::from(&internal_name)),
                MemberOptions {
                    default_method_flags: crate::classfile::ACC_PUBLIC
                        | crate::classfile::ACC_ABSTRACT,
                    lower_constructors: false,
                    ..MemberOptions::default()
                },
            )
        })
        .transpose()?
        .unwrap_or_default();

    Ok(TypeDecl {
        id: HirId(0),
        name: Ustr::from(&internal_name),
        kind: TypeDeclKind::Annotation,
        access_flags: access_flags
            | crate::classfile::ACC_INTERFACE
            | crate::classfile::ACC_ABSTRACT
            | crate::classfile::ACC_ANNOTATION,
        super_class: None,
        interfaces: vec![Ty::class("java/lang/annotation/Annotation")],
        type_params,
        generic_signature: None,
        fields: members.fields,
        methods: members.methods,
        inner_types: members.inner_types,
        anonymous: None,
        record_components: Vec::new(),
        annotations,
    })
}

pub(super) fn lower_annotation_uses(
    modifiers: Option<&JavaSyntaxNode>,
    package: Option<&Package>,
    resolver: &TypeResolver,
) -> LowerResult<Vec<AnnotationUse>> {
    let Some(modifiers) = modifiers else {
        return Ok(Vec::new());
    };

    modifiers
        .children()
        .filter(|child| child.kind() == JavaSyntaxKind::Annotation)
        .map(|annotation| lower_annotation_use(&annotation, package, resolver))
        .collect()
}

fn lower_annotation_use(
    annotation: &JavaSyntaxNode,
    package: Option<&Package>,
    resolver: &TypeResolver,
) -> LowerResult<AnnotationUse> {
    let name = qualified_name_text(annotation)?;
    let internal_name = resolver
        .resolve_class_reference(&name)
        .unwrap_or_else(|| fallback_annotation_name(package, &name));

    Ok(AnnotationUse {
        descriptor: format!("L{internal_name};"),
        elements: annotation_elements(annotation)?,
    })
}

fn fallback_annotation_name(package: Option<&Package>, name: &str) -> String {
    if name.contains('.') {
        return name.replace('.', "/");
    }
    internal_class_name(package, name)
}

fn annotation_elements(annotation: &JavaSyntaxNode) -> LowerResult<Vec<AnnotationElement>> {
    let Ok(tokens) = tokens_in_first_parens(annotation) else {
        return Ok(Vec::new());
    };
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    split_annotation_args(tokens)
        .into_iter()
        .map(lower_annotation_element)
        .collect()
}

fn split_annotation_args(tokens: Vec<ExprToken>) -> Vec<Vec<ExprToken>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0usize;

    for token in tokens {
        match token.kind {
            JavaSyntaxKind::LParen | JavaSyntaxKind::LBrace | JavaSyntaxKind::LBrack => {
                depth += 1;
                current.push(token);
            }
            JavaSyntaxKind::RParen | JavaSyntaxKind::RBrace | JavaSyntaxKind::RBrack => {
                depth = depth.saturating_sub(1);
                current.push(token);
            }
            JavaSyntaxKind::Comma if depth == 0 => {
                groups.push(std::mem::take(&mut current));
            }
            _ => current.push(token),
        }
    }

    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn lower_annotation_element(tokens: Vec<ExprToken>) -> LowerResult<AnnotationElement> {
    let (name, value_tokens) = split_named_annotation_value(&tokens);
    Ok(AnnotationElement {
        name: Ustr::from(name),
        value: lower_annotation_value(value_tokens)?,
    })
}

fn split_named_annotation_value(tokens: &[ExprToken]) -> (&str, &[ExprToken]) {
    let Some(eq_index) = tokens
        .iter()
        .position(|token| token.kind == JavaSyntaxKind::Eq)
    else {
        return ("value", tokens);
    };
    let name = tokens[..eq_index]
        .iter()
        .find(|token| token.kind == JavaSyntaxKind::Ident)
        .map(|token| token.text.as_str())
        .unwrap_or("value");
    (name, &tokens[eq_index + 1..])
}

fn lower_annotation_value(tokens: &[ExprToken]) -> LowerResult<AnnotationValue> {
    let Some(token) = tokens
        .iter()
        .find(|token| token.kind != JavaSyntaxKind::Comma)
    else {
        return Err(LowerError::UnsupportedExpression);
    };

    match token.kind {
        JavaSyntaxKind::StringLiteral => {
            Ok(AnnotationValue::String(string_literal_value(&token.text)))
        }
        JavaSyntaxKind::IntLiteral => Ok(AnnotationValue::Int(parse_int_literal(&token.text))),
        JavaSyntaxKind::TrueKw => Ok(AnnotationValue::Boolean(true)),
        JavaSyntaxKind::FalseKw => Ok(AnnotationValue::Boolean(false)),
        _ => Err(LowerError::UnsupportedExpression),
    }
}
