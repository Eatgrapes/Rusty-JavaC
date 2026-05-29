use crate::ast::{AstNode, ClassBody, FieldDecl as AstFieldDecl, MethodDecl as AstMethodDecl};
use crate::ast::{JavaSyntaxKind, JavaSyntaxNode};
use crate::hir::lowering::expr::BodyBuilder;
use crate::hir::lowering::modifiers::{access_flags, has_code};
use crate::hir::lowering::signature::{lower_type_params, method_signature};
use crate::hir::lowering::stmt::lower_block;
use crate::hir::lowering::syntax::{first_ident, initializer_tokens, last_ident, source_line};
use crate::hir::lowering::types::{TypeResolver, lower_type_with_vars};
use crate::hir::lowering::{LowerError, LowerResult};
use crate::hir::*;
use crate::ty::{MethodSig, Ty};
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use ustr::Ustr;

#[derive(Default)]
pub(super) struct ClassMembers {
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<MethodDecl>,
    pub inner_types: Vec<Rc<TypeDecl>>,
}

pub(super) fn lower_class_members(
    body: ClassBody,
    class_type_params: &[crate::ty::TypeParam],
    resolver: &TypeResolver,
    enclosing_static_owner: Option<Ustr>,
) -> LowerResult<ClassMembers> {
    let mut pending_flags = 0;
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut inner_types = Vec::new();
    let anonymous_counter = Rc::new(Cell::new(0));
    let type_vars = type_var_set(class_type_params, &[]);

    for child in body.syntax().children() {
        match child.kind() {
            JavaSyntaxKind::ModifierList => pending_flags = access_flags(&child),
            JavaSyntaxKind::FieldDecl => {
                let field = AstFieldDecl::cast(child).ok_or(LowerError::UnsupportedClassMember)?;
                let mut lowered = lower_field_decl(
                    field,
                    pending_flags,
                    fields.len() as u32,
                    &type_vars,
                    resolver,
                    BodyLoweringContext {
                        anonymous_counter: anonymous_counter.clone(),
                        enclosing_static_owner,
                        outer_fields: captured_fields(&fields),
                    },
                )?;
                inner_types.append(&mut lowered.inner_types);
                fields.append(&mut lowered.fields);
                pending_flags = 0;
            }
            JavaSyntaxKind::MethodDecl => {
                let method =
                    AstMethodDecl::cast(child).ok_or(LowerError::UnsupportedClassMember)?;
                let mut lowered = lower_method_decl(
                    method,
                    pending_flags,
                    methods.len() as u32,
                    class_type_params,
                    resolver,
                    BodyLoweringContext {
                        anonymous_counter: anonymous_counter.clone(),
                        enclosing_static_owner,
                        outer_fields: captured_fields(&fields),
                    },
                )?;
                inner_types.append(&mut lowered.inner_types);
                methods.push(lowered.method);
                pending_flags = 0;
            }
            JavaSyntaxKind::ConstructorDecl => {
                methods.push(lower_constructor_decl(
                    &child,
                    pending_flags,
                    methods.len() as u32,
                    class_type_params,
                    resolver,
                )?);
                pending_flags = 0;
            }
            JavaSyntaxKind::ClassDecl
            | JavaSyntaxKind::InterfaceDecl
            | JavaSyntaxKind::EnumDecl
            | JavaSyntaxKind::RecordDecl => pending_flags = 0,
            _ => {}
        }
    }

    Ok(ClassMembers {
        fields,
        methods,
        inner_types,
    })
}

struct LoweredFields {
    fields: Vec<FieldDecl>,
    inner_types: Vec<Rc<TypeDecl>>,
}

struct LoweredMethod {
    method: MethodDecl,
    inner_types: Vec<Rc<TypeDecl>>,
}

struct BodyLoweringContext {
    anonymous_counter: Rc<Cell<u32>>,
    enclosing_static_owner: Option<Ustr>,
    outer_fields: Vec<CapturedField>,
}

fn lower_field_decl(
    field: AstFieldDecl,
    access_flags: u16,
    first_field_index: u32,
    type_vars: &HashSet<Ustr>,
    resolver: &TypeResolver,
    body_context: BodyLoweringContext,
) -> LowerResult<LoweredFields> {
    let declared_ty = field.ty().ok_or(LowerError::MissingType)?;
    let ty = lower_type_with_vars(declared_ty.syntax(), type_vars, resolver)?;
    let mut fields = Vec::new();
    let mut inner_types = Vec::new();

    for declarator in var_declarators(field.syntax()) {
        let name = first_ident(&declarator).ok_or(LowerError::MissingMethodName)?;
        let mut body_builder = BodyBuilder::with_anonymous_context(
            resolver.clone(),
            body_context.anonymous_counter.clone(),
            access_flags & crate::classfile::ACC_STATIC == 0,
            body_context.enclosing_static_owner,
            body_context.outer_fields.clone(),
        );
        let initializer = initializer_tokens(&declarator)
            .map(|tokens| body_builder.lower_expr_tokens(&tokens))
            .transpose()?
            .flatten();
        inner_types.extend(body_builder.take_anonymous_types());

        fields.push(FieldDecl {
            id: HirId(first_field_index + fields.len() as u32 + 1),
            name: Ustr::from(name.text()),
            ty: ty.clone(),
            access_flags,
            generic_signature: None,
            body: body_builder.body,
            initializer,
        });
    }

    Ok(LoweredFields {
        fields,
        inner_types,
    })
}

fn var_declarators(field: &JavaSyntaxNode) -> Vec<JavaSyntaxNode> {
    field
        .children()
        .find(|node| node.kind() == JavaSyntaxKind::VarDeclaratorList)
        .into_iter()
        .flat_map(|list| {
            list.children()
                .filter(|node| node.kind() == JavaSyntaxKind::VarDeclarator)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn lower_constructor_decl(
    constructor: &JavaSyntaxNode,
    access_flags: u16,
    method_index: u32,
    class_type_params: &[crate::ty::TypeParam],
    resolver: &TypeResolver,
) -> LowerResult<MethodDecl> {
    let type_vars = type_var_set(class_type_params, &[]);
    let params = lower_method_params(constructor, &type_vars, resolver)?;
    let throws = lower_throws(constructor, &type_vars, resolver)?;
    let signature = MethodSig::new(
        Ustr::from("<init>"),
        params.iter().map(|param| param.ty.clone()).collect(),
        Ty::Void,
    );
    let mut body_builder = BodyBuilder::new(resolver.clone());
    define_params(&mut body_builder, &params);
    let root_block = constructor
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::MethodBody)
        .and_then(|body| {
            body.children()
                .find(|child| child.kind() == JavaSyntaxKind::Block)
        })
        .map(|block| lower_block(&block, &mut body_builder))
        .transpose()?;

    Ok(MethodDecl {
        id: HirId(method_index + 1),
        name: Ustr::from("<init>"),
        params,
        signature,
        access_flags,
        source_line: Some(source_line(constructor)),
        generic_signature: None,
        throws,
        body: body_builder.body,
        root_block,
        constructor_call: None,
    })
}

fn lower_method_decl(
    method: AstMethodDecl,
    access_flags: u16,
    method_index: u32,
    class_type_params: &[crate::ty::TypeParam],
    resolver: &TypeResolver,
    body_context: BodyLoweringContext,
) -> LowerResult<LoweredMethod> {
    let name = method.name().ok_or(LowerError::MissingMethodName)?;
    let method_type_params = lower_type_params(method.syntax(), resolver)?;
    let type_vars = type_var_set(class_type_params, &method_type_params);
    let return_type = method
        .return_type()
        .map(|ty| lower_type_with_vars(ty.syntax(), &type_vars, resolver))
        .transpose()?
        .unwrap_or(Ty::Void);
    let params = lower_method_params(method.syntax(), &type_vars, resolver)?;
    let throws = lower_throws(method.syntax(), &type_vars, resolver)?;
    let generic_signature = method_signature(
        method.syntax(),
        class_type_params,
        &method_type_params,
        resolver,
    )?;
    let mut signature = MethodSig::new(
        Ustr::from(name.text()),
        params.iter().map(|param| param.ty.clone()).collect(),
        return_type,
    );
    signature.type_params = method_type_params;
    let mut body_builder = BodyBuilder::with_anonymous_context(
        resolver.clone(),
        body_context.anonymous_counter,
        access_flags & crate::classfile::ACC_STATIC == 0,
        body_context.enclosing_static_owner,
        body_context.outer_fields,
    );
    define_params(&mut body_builder, &params);
    let root_block = lower_method_body(access_flags, &method, &mut body_builder)?;
    let ret_ty = signature.return_type.clone();
    body_builder.resolve_lambda_target_types(&ret_ty);
    let inner_types = body_builder.take_anonymous_types();

    Ok(LoweredMethod {
        method: MethodDecl {
            id: HirId(method_index + 1),
            name: Ustr::from(name.text()),
            params,
            signature,
            access_flags,
            source_line: Some(source_line(method.syntax())),
            generic_signature,
            throws,
            body: body_builder.body,
            root_block,
            constructor_call: None,
        },
        inner_types,
    })
}

fn captured_fields(fields: &[FieldDecl]) -> Vec<CapturedField> {
    fields
        .iter()
        .map(|field| CapturedField {
            name: field.name,
            ty: field.ty.clone(),
            access_flags: field.access_flags,
        })
        .collect()
}

fn lower_method_params(
    method: &JavaSyntaxNode,
    type_vars: &HashSet<Ustr>,
    resolver: &TypeResolver,
) -> LowerResult<Vec<ParamDecl>> {
    let Some(params) = method
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::FormalParamList)
    else {
        return Ok(Vec::new());
    };

    params
        .children()
        .filter(|child| child.kind() == JavaSyntaxKind::FormalParam)
        .map(|param| {
            let ty = param
                .children()
                .find(|child| child.kind() == JavaSyntaxKind::Type)
                .ok_or(LowerError::MissingType)?;
            let name = last_ident(&param).ok_or(LowerError::MissingMethodName)?;
            Ok(ParamDecl {
                name: Ustr::from(name.text()),
                ty: lower_type_with_vars(&ty, type_vars, resolver)?,
            })
        })
        .collect()
}

fn lower_throws(
    method: &JavaSyntaxNode,
    type_vars: &HashSet<Ustr>,
    resolver: &TypeResolver,
) -> LowerResult<Vec<Ty>> {
    let Some(throws_clause) = method
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::ThrowsClause)
    else {
        return Ok(Vec::new());
    };

    throws_clause
        .descendants()
        .filter(|node| node.kind() == JavaSyntaxKind::Type)
        .map(|ty| lower_type_with_vars(&ty, type_vars, resolver))
        .collect()
}

fn type_var_set(
    class_type_params: &[crate::ty::TypeParam],
    method_type_params: &[crate::ty::TypeParam],
) -> HashSet<Ustr> {
    class_type_params
        .iter()
        .chain(method_type_params)
        .map(|param| param.name)
        .collect()
}

fn define_params(body: &mut BodyBuilder, params: &[ParamDecl]) {
    for param in params {
        body.define_local(param.name, param.ty.clone());
    }
}

fn lower_method_body(
    access_flags: u16,
    method: &AstMethodDecl,
    body: &mut BodyBuilder,
) -> LowerResult<Option<Block>> {
    if has_code(access_flags)
        && let Some(method_body) = method.body()
    {
        method_body
            .block()
            .map(|block| lower_block(block.syntax(), body).map(Some))
            .unwrap_or(Ok(Some(Block { stmts: Vec::new() })))
    } else {
        Ok(None)
    }
}
