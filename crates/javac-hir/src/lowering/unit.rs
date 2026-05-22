use crate::hir::*;
use crate::lowering::member::lower_class_methods;
use crate::lowering::modifiers::access_flags;
use crate::lowering::syntax::qualified_name_text;
use crate::lowering::{LowerError, LowerResult};
use javac_ast::ast::{
    AstNode, ClassDecl, CompilationUnit as AstCompilationUnit, ImportDecl as AstImportDecl,
};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode};
use ustr::Ustr;

pub(super) fn lower_compilation_unit(node: &JavaSyntaxNode) -> LowerResult<CompilationUnit> {
    let unit = AstCompilationUnit::cast(node.clone()).ok_or(LowerError::ExpectedCompilationUnit)?;
    reject_unsupported_package(&unit)?;
    let imports = lower_imports(&unit)?;
    let type_decls = lower_top_level_types(node)?;

    Ok(CompilationUnit {
        package: None,
        imports,
        type_decls,
    })
}

fn reject_unsupported_package(unit: &AstCompilationUnit) -> LowerResult<()> {
    if unit.package().is_some() {
        return Err(LowerError::PackagesNotSupported);
    }
    Ok(())
}

fn lower_imports(unit: &AstCompilationUnit) -> LowerResult<Vec<Import>> {
    unit.imports().map(lower_import).collect()
}

fn lower_import(import: AstImportDecl) -> LowerResult<Import> {
    let path = qualified_name_text(import.syntax())?;
    Ok(Import {
        path: Ustr::from(&path),
        is_static: import.is_static(),
        is_wildcard: import.is_wildcard(),
    })
}

fn lower_top_level_types(node: &JavaSyntaxNode) -> LowerResult<Vec<TypeDecl>> {
    let mut pending_flags = 0;
    let mut type_decls = Vec::new();
    for child in node.children() {
        match child.kind() {
            JavaSyntaxKind::ModifierList => pending_flags = access_flags(&child),
            JavaSyntaxKind::ClassDecl => {
                let class = ClassDecl::cast(child).ok_or(LowerError::UnsupportedTypeDeclaration)?;
                type_decls.push(lower_class_decl(class, pending_flags)?);
                pending_flags = 0;
            }
            JavaSyntaxKind::InterfaceDecl
            | JavaSyntaxKind::EnumDecl
            | JavaSyntaxKind::RecordDecl
            | JavaSyntaxKind::AnnotationDecl => return Err(LowerError::UnsupportedTypeDeclaration),
            _ => {}
        }
    }

    if type_decls.len() != 1 {
        return Err(LowerError::ExpectedSingleTopLevelClass);
    }

    Ok(type_decls)
}

fn lower_class_decl(class: ClassDecl, access_flags: u16) -> LowerResult<TypeDecl> {
    let name = class.name().ok_or(LowerError::MissingClassName)?;
    let methods = class
        .body()
        .map(lower_class_methods)
        .transpose()?
        .unwrap_or_default();

    Ok(TypeDecl {
        id: HirId(0),
        name: Ustr::from(name.text()),
        kind: TypeDeclKind::Class,
        access_flags,
        super_class: None,
        interfaces: Vec::new(),
        type_params: Vec::new(),
        fields: Vec::new(),
        methods,
        inner_types: Vec::new(),
    })
}
