use crate::hir::*;
use crate::lowering::expr::BodyBuilder;
use crate::lowering::modifiers::{access_flags, has_code};
use crate::lowering::stmt::lower_block;
use crate::lowering::syntax::last_ident;
use crate::lowering::types::lower_type;
use crate::lowering::{LowerError, LowerResult};
use javac_ast::ast::{AstNode, ClassBody, MethodDecl as AstMethodDecl};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode};
use javac_ty::{MethodSig, Ty};
use ustr::Ustr;

pub(super) fn lower_class_methods(body: ClassBody) -> LowerResult<Vec<MethodDecl>> {
    let mut pending_flags = 0;
    let mut methods = Vec::new();

    for child in body.syntax().children() {
        match child.kind() {
            JavaSyntaxKind::ModifierList => pending_flags = access_flags(&child),
            JavaSyntaxKind::MethodDecl => {
                let method =
                    AstMethodDecl::cast(child).ok_or(LowerError::UnsupportedClassMember)?;
                methods.push(lower_method_decl(
                    method,
                    pending_flags,
                    methods.len() as u32,
                )?);
                pending_flags = 0;
            }
            JavaSyntaxKind::FieldDecl
            | JavaSyntaxKind::ConstructorDecl
            | JavaSyntaxKind::ClassDecl
            | JavaSyntaxKind::InterfaceDecl
            | JavaSyntaxKind::EnumDecl
            | JavaSyntaxKind::RecordDecl => return Err(LowerError::UnsupportedClassMember),
            _ => {}
        }
    }

    Ok(methods)
}

fn lower_method_decl(
    method: AstMethodDecl,
    access_flags: u16,
    method_index: u32,
) -> LowerResult<MethodDecl> {
    let name = method.name().ok_or(LowerError::MissingMethodName)?;
    let return_type = method
        .return_type()
        .map(|ty| lower_type(ty.syntax()))
        .transpose()?
        .unwrap_or(Ty::Void);
    let params = lower_method_params(method.syntax())?;
    let signature = MethodSig::new(
        Ustr::from(name.text()),
        params.iter().map(|param| param.ty.clone()).collect(),
        return_type,
    );
    let mut body_builder = BodyBuilder::default();
    define_params(&mut body_builder, &params);
    let root_block = lower_method_body(access_flags, &method, &mut body_builder)?;

    Ok(MethodDecl {
        id: HirId(method_index + 1),
        name: Ustr::from(name.text()),
        params,
        signature,
        access_flags,
        body: body_builder.body,
        root_block,
    })
}

fn lower_method_params(method: &JavaSyntaxNode) -> LowerResult<Vec<ParamDecl>> {
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
                ty: lower_type(&ty)?,
            })
        })
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
