use crate::hir::*;
use javac_ast::ast::{
    AstNode, ClassBody, ClassDecl, CompilationUnit as AstCompilationUnit,
    ImportDecl as AstImportDecl, MethodDecl as AstMethodDecl,
};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode, JavaSyntaxToken};
use javac_ty::{MethodSig, Ty};
use std::fmt;
use ustr::Ustr;

const ACC_PUBLIC: u16 = 0x0001;
const ACC_PRIVATE: u16 = 0x0002;
const ACC_PROTECTED: u16 = 0x0004;
const ACC_STATIC: u16 = 0x0008;
const ACC_FINAL: u16 = 0x0010;
const ACC_SYNCHRONIZED: u16 = 0x0020;
const ACC_NATIVE: u16 = 0x0100;
const ACC_ABSTRACT: u16 = 0x0400;

pub type LowerResult<T> = Result<T, LowerError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    ExpectedCompilationUnit,
    PackagesNotSupported,
    UnsupportedTypeDeclaration,
    ExpectedSingleTopLevelClass,
    UnsupportedClassMember,
    MissingClassName,
    MissingImportName,
    MissingMethodName,
    MissingType,
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            LowerError::ExpectedCompilationUnit => "expected compilation unit",
            LowerError::PackagesNotSupported => "packages are not supported yet",
            LowerError::UnsupportedTypeDeclaration => "only class declarations are supported yet",
            LowerError::ExpectedSingleTopLevelClass => "expected one top-level class",
            LowerError::UnsupportedClassMember => "unsupported class member",
            LowerError::MissingClassName => "class declaration is missing a name",
            LowerError::MissingImportName => "import declaration is missing a name",
            LowerError::MissingMethodName => "method declaration is missing a name",
            LowerError::MissingType => "type syntax is missing",
        };
        f.write_str(message)
    }
}

pub fn lower(node: &JavaSyntaxNode) -> LowerResult<CompilationUnit> {
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

fn lower_class_methods(body: ClassBody) -> LowerResult<Vec<MethodDecl>> {
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
    let signature = MethodSig::new(Ustr::from(name.text()), params, return_type);

    Ok(MethodDecl {
        id: HirId(method_index + 1),
        name: Ustr::from(name.text()),
        signature,
        access_flags,
        body: Body::default(),
        root_block: method_body(access_flags, &method),
    })
}

fn lower_method_params(method: &JavaSyntaxNode) -> LowerResult<Vec<Ty>> {
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
            lower_type(&ty)
        })
        .collect()
}

fn method_body(access_flags: u16, method: &AstMethodDecl) -> Option<Block> {
    let has_code = access_flags & (ACC_ABSTRACT | ACC_NATIVE) == 0;
    if has_code && method.body().is_some() {
        Some(Block { stmts: Vec::new() })
    } else {
        None
    }
}

fn lower_type(node: &JavaSyntaxNode) -> LowerResult<Ty> {
    let mut base = lower_base_type(node)?;
    for _ in 0..array_dimensions(node) {
        base = Ty::Array(Box::new(base));
    }
    Ok(base)
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
        JavaSyntaxKind::Ident => Ty::Class(Ustr::from(&class_internal_name(token.text()))),
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

fn qualified_name_text(node: &JavaSyntaxNode) -> LowerResult<String> {
    let Some(name) = node
        .descendants()
        .find(|child| child.kind() == JavaSyntaxKind::QualifiedName)
    else {
        return Err(LowerError::MissingImportName);
    };

    let text = name
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| matches!(token.kind(), JavaSyntaxKind::Ident | JavaSyntaxKind::Dot))
        .map(|token| token.text().to_string())
        .collect::<String>();

    if text.is_empty() {
        Err(LowerError::MissingImportName)
    } else {
        Ok(text)
    }
}

fn access_flags(node: &JavaSyntaxNode) -> u16 {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .fold(0, |flags, token| match token.kind() {
            JavaSyntaxKind::PublicKw => flags | ACC_PUBLIC,
            JavaSyntaxKind::PrivateKw => flags | ACC_PRIVATE,
            JavaSyntaxKind::ProtectedKw => flags | ACC_PROTECTED,
            JavaSyntaxKind::StaticKw => flags | ACC_STATIC,
            JavaSyntaxKind::FinalKw => flags | ACC_FINAL,
            JavaSyntaxKind::SynchronizedKw => flags | ACC_SYNCHRONIZED,
            JavaSyntaxKind::NativeKw => flags | ACC_NATIVE,
            JavaSyntaxKind::AbstractKw => flags | ACC_ABSTRACT,
            _ => flags,
        })
}
