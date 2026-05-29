use crate::ast::{
    AstNode, ClassDecl, CompilationUnit as AstCompilationUnit, ImportDecl as AstImportDecl,
    TypeDecl as AstTypeDecl,
};
use crate::ast::{JavaSyntaxKind, JavaSyntaxNode};
use crate::call_resolver::ClassCatalog;
use crate::hir::lowering::member::lower_class_members;
use crate::hir::lowering::modifiers::access_flags;
use crate::hir::lowering::signature::{class_signature, lower_type_params};
use crate::hir::lowering::syntax::{qualified_name_text, qualified_name_text_range, source_line};
use crate::hir::lowering::types::TypeResolver;
use crate::hir::lowering::{LowerError, LowerResult};
use crate::hir::*;
use ustr::Ustr;

pub(super) fn lower_compilation_unit(
    node: &JavaSyntaxNode,
    catalog: &ClassCatalog,
) -> LowerResult<CompilationUnit> {
    let unit = AstCompilationUnit::cast(node.clone()).ok_or(LowerError::ExpectedCompilationUnit)?;
    let package = lower_package(&unit)?;
    let imports = lower_imports(&unit)?;
    let type_decls = lower_top_level_types(node, package.as_ref(), &imports, catalog)?;

    Ok(CompilationUnit {
        package,
        imports,
        type_decls,
    })
}

fn lower_package(unit: &AstCompilationUnit) -> LowerResult<Option<Package>> {
    unit.package()
        .map(|package| {
            let name = qualified_name_text(package.syntax())?;
            Ok(Package {
                name: Ustr::from(&name),
            })
        })
        .transpose()
}

fn lower_imports(unit: &AstCompilationUnit) -> LowerResult<Vec<Import>> {
    unit.imports().map(lower_import).collect()
}

fn lower_import(import: AstImportDecl) -> LowerResult<Import> {
    let (path, range) = qualified_name_text_range(import.syntax())?;
    Ok(Import {
        path: Ustr::from(&path),
        is_static: import.is_static(),
        is_wildcard: import.is_wildcard(),
        source_line: Some(source_line(import.syntax())),
        source_range: Some(range),
    })
}

fn lower_top_level_types(
    node: &JavaSyntaxNode,
    package: Option<&Package>,
    imports: &[Import],
    catalog: &ClassCatalog,
) -> LowerResult<Vec<TypeDecl>> {
    let mut pending_flags = 0;
    let mut pending_modifiers = None;
    let mut type_decls = Vec::new();
    for child in node.children() {
        match child.kind() {
            JavaSyntaxKind::ModifierList => {
                pending_flags = access_flags(&child);
                pending_modifiers = Some(child);
            }
            JavaSyntaxKind::ClassDecl
            | JavaSyntaxKind::InterfaceDecl
            | JavaSyntaxKind::EnumDecl
            | JavaSyntaxKind::RecordDecl
            | JavaSyntaxKind::AnnotationDecl => {
                let decl =
                    AstTypeDecl::cast(child).ok_or(LowerError::UnsupportedTypeDeclaration)?;
                type_decls.push(lower_type_decl(
                    decl,
                    pending_flags,
                    pending_modifiers.as_ref(),
                    package,
                    imports,
                    catalog,
                )?);
                pending_flags = 0;
                pending_modifiers = None;
            }
            _ => {}
        }
    }

    if type_decls.is_empty() {
        return Err(LowerError::ExpectedSingleTopLevelClass);
    }

    Ok(type_decls)
}

fn lower_type_decl(
    decl: AstTypeDecl,
    access_flags: u16,
    modifiers: Option<&JavaSyntaxNode>,
    package: Option<&Package>,
    imports: &[Import],
    catalog: &ClassCatalog,
) -> LowerResult<TypeDecl> {
    match decl {
        AstTypeDecl::Class(class) => {
            lower_class_decl(class, access_flags, modifiers, package, imports, catalog)
        }
        AstTypeDecl::Record(record) => crate::hir::lowering::record::lower_record_decl(
            record,
            access_flags,
            modifiers,
            package,
            imports,
            catalog,
        ),
        AstTypeDecl::Enum(enum_decl) => crate::hir::lowering::enum_decl::lower_enum_decl(
            enum_decl,
            access_flags,
            modifiers,
            package,
            imports,
            catalog,
        ),
        AstTypeDecl::Annotation(annotation) => {
            crate::hir::lowering::annotation::lower_annotation_decl(
                annotation,
                access_flags,
                modifiers,
                package,
                imports,
                catalog,
            )
        }
        AstTypeDecl::Interface(_) => Err(LowerError::UnsupportedTypeDeclaration),
    }
}

fn lower_class_decl(
    class: ClassDecl,
    access_flags: u16,
    modifiers: Option<&JavaSyntaxNode>,
    package: Option<&Package>,
    imports: &[Import],
    catalog: &ClassCatalog,
) -> LowerResult<TypeDecl> {
    let name = class.name().ok_or(LowerError::MissingClassName)?;
    let internal_name = internal_class_name(package, name.text());
    let resolver = TypeResolver::for_class(package, imports, &internal_name, catalog)?;
    let annotations =
        crate::hir::lowering::annotation::lower_annotation_uses(modifiers, package, &resolver)?;
    let type_params = lower_type_params(class.syntax(), &resolver)?;
    let generic_signature = class_signature(class.syntax(), &type_params, &resolver)?;
    let members = class
        .body()
        .map(|body| {
            lower_class_members(
                body,
                &type_params,
                &resolver,
                Some(Ustr::from(&internal_name)),
            )
        })
        .transpose()?
        .unwrap_or_default();

    Ok(TypeDecl {
        id: HirId(0),
        name: Ustr::from(&internal_name),
        kind: TypeDeclKind::Class,
        access_flags,
        super_class: None,
        interfaces: Vec::new(),
        type_params,
        generic_signature,
        fields: members.fields,
        methods: members.methods,
        inner_types: members.inner_types,
        anonymous: None,
        record_components: Vec::new(),
        annotations,
    })
}

pub(super) fn internal_class_name(package: Option<&Package>, simple_name: &str) -> String {
    match package {
        Some(package) => format!(
            "{}/{}",
            package.name.as_str().replace('.', "/"),
            simple_name
        ),
        None => simple_name.to_string(),
    }
}
