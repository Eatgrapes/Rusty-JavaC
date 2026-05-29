use crate::ast::{AstNode, EnumDecl, JavaSyntaxKind, JavaSyntaxNode};
use crate::call_resolver::ClassCatalog;
use crate::hir::lowering::expr::BodyBuilder;
use crate::hir::lowering::member::{MemberOptions, lower_members};
use crate::hir::lowering::signature::{class_signature, lower_type_params};
use crate::hir::lowering::syntax::{ExprToken, first_ident, tokens_in_first_parens};
use crate::hir::lowering::types::TypeResolver;
use crate::hir::lowering::unit::internal_class_name;
use crate::hir::lowering::{LowerError, LowerResult};
use crate::hir::*;
use crate::ty::{MethodSig, Ty};
use ustr::Ustr;

const ENUM_VALUES_FIELD: &str = "$VALUES";

pub(super) fn lower_enum_decl(
    enum_decl: EnumDecl,
    access_flags: u16,
    modifiers: Option<&JavaSyntaxNode>,
    package: Option<&Package>,
    imports: &[Import],
    catalog: &ClassCatalog,
) -> LowerResult<TypeDecl> {
    let name = enum_decl.name().ok_or(LowerError::MissingClassName)?;
    let internal_name = internal_class_name(package, name.text());
    let resolver = TypeResolver::for_class(package, imports, &internal_name, catalog)?;
    let annotations =
        crate::hir::lowering::annotation::lower_annotation_uses(modifiers, package, &resolver)?;
    let type_params = lower_type_params(enum_decl.syntax(), &resolver)?;
    let generic_signature = class_signature(enum_decl.syntax(), &type_params, &resolver)?;
    let constants = enum_constants(enum_decl.syntax(), &internal_name, &resolver)?;

    let mut members = enum_decl
        .body()
        .map(|body| {
            lower_members(
                body.syntax(),
                &type_params,
                &resolver,
                Some(Ustr::from(&internal_name)),
                MemberOptions::class(),
            )
        })
        .transpose()?
        .unwrap_or_default();

    let mut fields = constants;
    fields.append(&mut members.fields);
    fields.push(enum_values_field(&internal_name, fields.len() as u32 + 1));

    let mut methods = members.methods;
    prepare_enum_constructors(&mut methods);
    if !methods.iter().any(|method| method.name == "<init>") {
        methods.push(default_enum_constructor(methods.len() as u32 + 1));
    }
    methods.push(enum_values_method(&internal_name, methods.len() as u32 + 1));
    methods.push(enum_value_of_method(
        &internal_name,
        methods.len() as u32 + 1,
    ));

    Ok(TypeDecl {
        id: HirId(0),
        name: Ustr::from(&internal_name),
        kind: TypeDeclKind::Enum,
        access_flags: access_flags | crate::classfile::ACC_FINAL | crate::classfile::ACC_ENUM,
        super_class: Some(Ty::class("java/lang/Enum")),
        interfaces: Vec::new(),
        type_params,
        generic_signature,
        fields,
        methods,
        inner_types: members.inner_types,
        anonymous: None,
        record_components: Vec::new(),
        annotations,
    })
}

fn enum_constants(
    enum_decl: &JavaSyntaxNode,
    owner: &str,
    resolver: &TypeResolver,
) -> LowerResult<Vec<FieldDecl>> {
    let Some(list) = enum_decl
        .descendants()
        .find(|node| node.kind() == JavaSyntaxKind::EnumConstantList)
    else {
        return Ok(Vec::new());
    };

    list.children()
        .filter(|child| child.kind() == JavaSyntaxKind::EnumConstant)
        .enumerate()
        .map(|(index, constant)| enum_constant_field(&constant, owner, resolver, index))
        .collect()
}

fn enum_constant_field(
    constant: &JavaSyntaxNode,
    owner: &str,
    resolver: &TypeResolver,
    index: usize,
) -> LowerResult<FieldDecl> {
    let name = first_ident(constant).ok_or(LowerError::MissingMethodName)?;
    let mut body = BodyBuilder::new(resolver.clone());
    let mut args = vec![
        body.alloc_expr(Expr::StringLiteral(Ustr::from(name.text()))),
        body.alloc_expr(Expr::IntLiteral(index as i64)),
    ];
    args.extend(enum_constant_args(constant, &mut body)?);
    let initializer = body.alloc_expr(Expr::NewObject {
        class: Ty::class(owner),
        args,
        anonymous: None,
    });

    Ok(FieldDecl {
        id: HirId(index as u32 + 1),
        name: Ustr::from(name.text()),
        ty: Ty::class(owner),
        access_flags: crate::classfile::ACC_PUBLIC
            | crate::classfile::ACC_STATIC
            | crate::classfile::ACC_FINAL
            | crate::classfile::ACC_ENUM,
        generic_signature: None,
        body: body.body,
        initializer: Some(initializer),
    })
}

fn enum_constant_args(
    constant: &JavaSyntaxNode,
    body: &mut BodyBuilder,
) -> LowerResult<Vec<ExprId>> {
    argument_token_groups(constant)
        .into_iter()
        .map(|tokens| {
            body.lower_expr_tokens(&tokens)?
                .ok_or(LowerError::UnsupportedExpression)
        })
        .collect()
}

fn argument_token_groups(node: &JavaSyntaxNode) -> Vec<Vec<ExprToken>> {
    let Ok(tokens) = tokens_in_first_parens(node) else {
        return Vec::new();
    };
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0usize;

    for token in tokens {
        match token.kind {
            JavaSyntaxKind::LParen => {
                depth += 1;
                current.push(token);
            }
            JavaSyntaxKind::RParen => {
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

fn enum_values_field(owner: &str, id: u32) -> FieldDecl {
    FieldDecl {
        id: HirId(id),
        name: Ustr::from(ENUM_VALUES_FIELD),
        ty: Ty::Array(Box::new(Ty::class(owner))),
        access_flags: crate::classfile::ACC_PRIVATE
            | crate::classfile::ACC_STATIC
            | crate::classfile::ACC_FINAL
            | crate::classfile::ACC_SYNTHETIC,
        generic_signature: None,
        body: Body::default(),
        initializer: None,
    }
}

fn prepare_enum_constructors(methods: &mut [MethodDecl]) {
    for method in methods.iter_mut().filter(|method| method.name == "<init>") {
        let mut params = enum_synthetic_params();
        params.extend(method.params.clone());
        method.params = params;
        method.signature.params = method.params.iter().map(|param| param.ty.clone()).collect();
        method.access_flags = private_access(method.access_flags);
        method.constructor_call = Some(enum_super_call());
    }
}

fn default_enum_constructor(id: u32) -> MethodDecl {
    let params = enum_synthetic_params();
    MethodDecl {
        id: HirId(id),
        name: Ustr::from("<init>"),
        params: params.clone(),
        signature: MethodSig::new(
            Ustr::from("<init>"),
            params.iter().map(|param| param.ty.clone()).collect(),
            Ty::Void,
        ),
        access_flags: crate::classfile::ACC_PRIVATE,
        source_line: Some(1),
        generic_signature: None,
        throws: Vec::new(),
        body: Body::default(),
        root_block: Some(Block { stmts: Vec::new() }),
        constructor_call: Some(enum_super_call()),
    }
}

fn enum_synthetic_params() -> Vec<ParamDecl> {
    vec![
        ParamDecl {
            name: Ustr::from("$name"),
            ty: Ty::string(),
        },
        ParamDecl {
            name: Ustr::from("$ordinal"),
            ty: Ty::Int,
        },
    ]
}

fn enum_super_call() -> SuperConstructorCall {
    SuperConstructorCall {
        owner: Ty::class("java/lang/Enum"),
        params: vec![Ty::string(), Ty::Int],
        arg_offset: 0,
    }
}

fn private_access(flags: u16) -> u16 {
    let access_mask = crate::classfile::ACC_PUBLIC
        | crate::classfile::ACC_PROTECTED
        | crate::classfile::ACC_PRIVATE;
    (flags & !access_mask) | crate::classfile::ACC_PRIVATE
}

fn enum_values_method(owner: &str, id: u32) -> MethodDecl {
    let return_ty = Ty::Array(Box::new(Ty::class(owner)));
    MethodDecl {
        id: HirId(id),
        name: Ustr::from("values"),
        params: Vec::new(),
        signature: MethodSig::new(Ustr::from("values"), Vec::new(), return_ty),
        access_flags: crate::classfile::ACC_PUBLIC | crate::classfile::ACC_STATIC,
        source_line: Some(1),
        generic_signature: None,
        throws: Vec::new(),
        body: Body::default(),
        root_block: None,
        constructor_call: None,
    }
}

fn enum_value_of_method(owner: &str, id: u32) -> MethodDecl {
    let params = vec![ParamDecl {
        name: Ustr::from("name"),
        ty: Ty::string(),
    }];
    MethodDecl {
        id: HirId(id),
        name: Ustr::from("valueOf"),
        params: params.clone(),
        signature: MethodSig::new(
            Ustr::from("valueOf"),
            params.iter().map(|param| param.ty.clone()).collect(),
            Ty::class(owner),
        ),
        access_flags: crate::classfile::ACC_PUBLIC | crate::classfile::ACC_STATIC,
        source_line: Some(1),
        generic_signature: None,
        throws: Vec::new(),
        body: Body::default(),
        root_block: None,
        constructor_call: None,
    }
}
