use crate::ast::{AstNode, JavaSyntaxKind, JavaSyntaxNode, RecordDecl};
use crate::call_resolver::ClassCatalog;
use crate::hir::lowering::expr::BodyBuilder;
use crate::hir::lowering::member::{MemberOptions, lower_members};
use crate::hir::lowering::signature::{class_signature, lower_type_params};
use crate::hir::lowering::stmt::lower_block;
use crate::hir::lowering::syntax::{last_ident, source_line};
use crate::hir::lowering::types::{TypeResolver, lower_type_with_vars};
use crate::hir::lowering::unit::internal_class_name;
use crate::hir::lowering::{LowerError, LowerResult};
use crate::hir::*;
use crate::ty::{MethodSig, Ty};
use std::collections::HashSet;
use ustr::Ustr;

pub(super) fn lower_record_decl(
    record: RecordDecl,
    access_flags: u16,
    modifiers: Option<&JavaSyntaxNode>,
    package: Option<&Package>,
    imports: &[Import],
    catalog: &ClassCatalog,
) -> LowerResult<TypeDecl> {
    let name = record.name().ok_or(LowerError::MissingClassName)?;
    let internal_name = internal_class_name(package, name.text());
    let resolver = TypeResolver::for_class(package, imports, &internal_name, catalog)?;
    let annotations =
        crate::hir::lowering::annotation::lower_annotation_uses(modifiers, package, &resolver)?;
    let type_params = lower_type_params(record.syntax(), &resolver)?;
    let generic_signature = class_signature(record.syntax(), &type_params, &resolver)?;
    let type_vars = type_params
        .iter()
        .map(|param| param.name)
        .collect::<HashSet<_>>();
    let components = lower_record_components(record.syntax(), &type_vars, &resolver)?;

    let mut members = record
        .body()
        .map(|body| {
            lower_members(
                body.syntax(),
                &type_params,
                &resolver,
                Some(Ustr::from(&internal_name)),
                MemberOptions {
                    lower_constructors: false,
                    ..MemberOptions::default()
                },
            )
        })
        .transpose()?
        .unwrap_or_default();

    let mut fields = record_component_fields(&components);
    fields.append(&mut members.fields);

    let mut methods = Vec::new();
    methods.push(record_constructor(
        record.syntax(),
        record.body().as_ref().map(AstNode::syntax),
        &components,
        &resolver,
    )?);
    methods.extend(record_accessors(&components, &members.methods));
    methods.append(&mut members.methods);

    Ok(TypeDecl {
        id: HirId(0),
        name: Ustr::from(&internal_name),
        kind: TypeDeclKind::Record,
        access_flags: access_flags | crate::classfile::ACC_FINAL,
        super_class: Some(Ty::class("java/lang/Record")),
        interfaces: Vec::new(),
        type_params,
        generic_signature,
        fields,
        methods,
        inner_types: members.inner_types,
        anonymous: None,
        record_components: components,
        annotations,
    })
}

fn lower_record_components(
    record: &JavaSyntaxNode,
    type_vars: &HashSet<Ustr>,
    resolver: &TypeResolver,
) -> LowerResult<Vec<RecordComponentDecl>> {
    let Some(list) = record
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::RecordComponentList)
    else {
        return Ok(Vec::new());
    };

    list.children()
        .filter(|child| child.kind() == JavaSyntaxKind::RecordComponent)
        .map(|component| {
            let ty_node = component
                .children()
                .find(|child| child.kind() == JavaSyntaxKind::Type)
                .ok_or(LowerError::MissingType)?;
            let name = last_ident(&component).ok_or(LowerError::MissingMethodName)?;
            Ok(RecordComponentDecl {
                name: Ustr::from(name.text()),
                ty: lower_type_with_vars(&ty_node, type_vars, resolver)?,
                generic_signature: None,
            })
        })
        .collect()
}

fn record_component_fields(components: &[RecordComponentDecl]) -> Vec<FieldDecl> {
    components
        .iter()
        .enumerate()
        .map(|(index, component)| FieldDecl {
            id: HirId(index as u32 + 1),
            name: component.name,
            ty: component.ty.clone(),
            access_flags: crate::classfile::ACC_PRIVATE | crate::classfile::ACC_FINAL,
            generic_signature: component.generic_signature.clone(),
            body: Body::default(),
            initializer: None,
        })
        .collect()
}

fn record_constructor(
    record: &JavaSyntaxNode,
    body: Option<&JavaSyntaxNode>,
    components: &[RecordComponentDecl],
    resolver: &TypeResolver,
) -> LowerResult<MethodDecl> {
    let params = components
        .iter()
        .map(|component| ParamDecl {
            name: component.name,
            ty: component.ty.clone(),
        })
        .collect::<Vec<_>>();
    let mut body_builder = BodyBuilder::new(resolver.clone());
    for param in &params {
        body_builder.define_local(param.name, param.ty.clone());
    }

    let mut stmts = compact_constructor_stmts(body, &mut body_builder)?;
    for component in components {
        stmts.push(assign_component_stmt(
            &mut body_builder,
            component,
            source_line(record),
        ));
    }

    let signature = MethodSig::new(
        Ustr::from("<init>"),
        params.iter().map(|param| param.ty.clone()).collect(),
        Ty::Void,
    );

    Ok(MethodDecl {
        id: HirId(1),
        name: Ustr::from("<init>"),
        params,
        signature,
        access_flags: crate::classfile::ACC_PUBLIC,
        source_line: Some(source_line(record)),
        generic_signature: None,
        throws: Vec::new(),
        body: body_builder.body,
        root_block: Some(Block { stmts }),
        constructor_call: None,
    })
}

fn compact_constructor_stmts(
    body: Option<&JavaSyntaxNode>,
    body_builder: &mut BodyBuilder,
) -> LowerResult<Vec<StmtId>> {
    let Some(constructor) = body
        .into_iter()
        .flat_map(JavaSyntaxNode::children)
        .find(|child| {
            child.kind() == JavaSyntaxKind::ConstructorDecl
                && child
                    .children()
                    .all(|node| node.kind() != JavaSyntaxKind::FormalParamList)
        })
    else {
        return Ok(Vec::new());
    };
    let Some(block) = constructor
        .descendants()
        .find(|node| node.kind() == JavaSyntaxKind::Block)
    else {
        return Ok(Vec::new());
    };
    Ok(lower_block(&block, body_builder)?.stmts)
}

fn assign_component_stmt(
    body: &mut BodyBuilder,
    component: &RecordComponentDecl,
    line: u16,
) -> StmtId {
    let this = body.alloc_expr(Expr::This);
    let target = body.alloc_expr(Expr::FieldAccess {
        target: this,
        field: component.name,
    });
    let value = body.alloc_expr(Expr::Ident(component.name));
    let assign = body.alloc_expr(Expr::Assign {
        target,
        op: AssignOp::Plain,
        value,
    });
    body.alloc_stmt_at(Stmt::Expr(assign), line)
}

fn record_accessors(
    components: &[RecordComponentDecl],
    existing_methods: &[MethodDecl],
) -> Vec<MethodDecl> {
    components
        .iter()
        .enumerate()
        .filter(|(_, component)| {
            !existing_methods
                .iter()
                .any(|method| method.name == component.name && method.params.is_empty())
        })
        .map(|(index, component)| record_accessor(component, index as u32 + 2))
        .collect()
}

fn record_accessor(component: &RecordComponentDecl, id: u32) -> MethodDecl {
    let mut body = Body::default();
    let this = body.exprs.alloc(Expr::This);
    let field = body.exprs.alloc(Expr::FieldAccess {
        target: this,
        field: component.name,
    });
    let stmt = body.stmts.alloc(Stmt::Return(Some(field)));
    body.stmt_lines.insert(stmt, 1);

    MethodDecl {
        id: HirId(id),
        name: component.name,
        params: Vec::new(),
        signature: MethodSig::new(component.name, Vec::new(), component.ty.clone()),
        access_flags: crate::classfile::ACC_PUBLIC,
        source_line: Some(1),
        generic_signature: None,
        throws: Vec::new(),
        body,
        root_block: Some(Block { stmts: vec![stmt] }),
        constructor_call: None,
    }
}
