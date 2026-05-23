use crate::hir::*;
use crate::lowering::expr::BodyBuilder;
use crate::lowering::modifiers::{access_flags, has_code};
use crate::lowering::signature::{lower_type_params, method_signature};
use crate::lowering::stmt::lower_block;
use crate::lowering::syntax::{first_ident, initializer_tokens, last_ident, source_line};
use crate::lowering::types::lower_type_with_vars;
use crate::lowering::{LowerError, LowerResult};
use javac_ast::ast::{
    AstNode, ClassBody, FieldDecl as AstFieldDecl, MethodDecl as AstMethodDecl,
};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode};
use javac_ty::{MethodSig, Ty};
use std::collections::HashSet;
use ustr::Ustr;

#[derive(Default)]
pub(super) struct ClassMembers {
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<MethodDecl>,
}

pub(super) fn lower_class_members(
    body: ClassBody,
    class_type_params: &[javac_ty::TypeParam],
) -> LowerResult<ClassMembers> {
    let mut pending_flags = 0;
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let type_vars = type_var_set(class_type_params, &[]);

    for child in body.syntax().children() {
        match child.kind() {
            JavaSyntaxKind::ModifierList => pending_flags = access_flags(&child),
            JavaSyntaxKind::FieldDecl => {
                let field = AstFieldDecl::cast(child).ok_or(LowerError::UnsupportedClassMember)?;
                fields.extend(lower_field_decl(
                    field,
                    pending_flags,
                    fields.len() as u32,
                    &type_vars,
                )?);
                pending_flags = 0;
            }
            JavaSyntaxKind::MethodDecl => {
                let method =
                    AstMethodDecl::cast(child).ok_or(LowerError::UnsupportedClassMember)?;
                methods.push(lower_method_decl(
                    method,
                    pending_flags,
                    methods.len() as u32,
                    class_type_params,
                )?);
                pending_flags = 0;
            }
            JavaSyntaxKind::ConstructorDecl
            | JavaSyntaxKind::ClassDecl
            | JavaSyntaxKind::InterfaceDecl
            | JavaSyntaxKind::EnumDecl
            | JavaSyntaxKind::RecordDecl => return Err(LowerError::UnsupportedClassMember),
            _ => {}
        }
    }

    Ok(ClassMembers { fields, methods })
}

fn lower_field_decl(
    field: AstFieldDecl,
    access_flags: u16,
    first_field_index: u32,
    type_vars: &HashSet<Ustr>,
) -> LowerResult<Vec<FieldDecl>> {
    let declared_ty = field.ty().ok_or(LowerError::MissingType)?;
    let ty = lower_type_with_vars(declared_ty.syntax(), type_vars)?;
    let mut fields = Vec::new();

    for declarator in field
        .syntax()
        .descendants()
        .filter(|node| node.kind() == JavaSyntaxKind::VarDeclarator)
    {
        let name = first_ident(&declarator).ok_or(LowerError::MissingMethodName)?;
        let mut body_builder = BodyBuilder::default();
        let initializer = initializer_tokens(&declarator)
            .map(|tokens| body_builder.lower_expr_tokens(&tokens))
            .transpose()?
            .flatten();

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

    Ok(fields)
}

fn lower_method_decl(
    method: AstMethodDecl,
    access_flags: u16,
    method_index: u32,
    class_type_params: &[javac_ty::TypeParam],
) -> LowerResult<MethodDecl> {
    let name = method.name().ok_or(LowerError::MissingMethodName)?;
    let method_type_params = lower_type_params(method.syntax())?;
    let type_vars = type_var_set(class_type_params, &method_type_params);
    let return_type = method
        .return_type()
        .map(|ty| lower_type_with_vars(ty.syntax(), &type_vars))
        .transpose()?
        .unwrap_or(Ty::Void);
    let params = lower_method_params(method.syntax(), &type_vars)?;
    let generic_signature =
        method_signature(method.syntax(), class_type_params, &method_type_params)?;
    let mut signature = MethodSig::new(
        Ustr::from(name.text()),
        params.iter().map(|param| param.ty.clone()).collect(),
        return_type,
    );
    signature.type_params = method_type_params;
    let mut body_builder = BodyBuilder::default();
    define_params(&mut body_builder, &params);
    let root_block = lower_method_body(access_flags, &method, &mut body_builder)?;

    Ok(MethodDecl {
        id: HirId(method_index + 1),
        name: Ustr::from(name.text()),
        params,
        signature,
        access_flags,
        source_line: Some(source_line(method.syntax())),
        generic_signature,
        body: body_builder.body,
        root_block,
    })
}

fn lower_method_params(
    method: &JavaSyntaxNode,
    type_vars: &HashSet<Ustr>,
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
                ty: lower_type_with_vars(&ty, type_vars)?,
            })
        })
        .collect()
}

fn type_var_set(
    class_type_params: &[javac_ty::TypeParam],
    method_type_params: &[javac_ty::TypeParam],
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
