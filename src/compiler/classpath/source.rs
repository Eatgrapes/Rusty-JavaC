use crate::ast::{
    AstNode, ClassDecl as AstClassDecl, CompilationUnit as AstCompilationUnit,
    FieldDecl as AstFieldDecl, InterfaceDecl as AstInterfaceDecl, MethodDecl as AstMethodDecl,
    TypeDecl as AstTypeDecl,
};
use crate::ast::{JavaSyntaxKind, JavaSyntaxNode};
use crate::call_resolver::ClassCatalog;
use crate::lexer::Lexer;
use crate::ty::{MethodSig, Ty};
use std::collections::{HashMap, HashSet};
use ustr::Ustr;

const ACC_PUBLIC: u16 = 0x0001;
const ACC_PRIVATE: u16 = 0x0002;
const ACC_PROTECTED: u16 = 0x0004;
const ACC_STATIC: u16 = 0x0008;
const ACC_FINAL: u16 = 0x0010;
const ACC_VARARGS: u16 = 0x0080;
const ACC_ABSTRACT: u16 = 0x0400;

#[derive(Clone)]
pub(super) struct JavaSource {
    pub label: String,
    pub source: String,
}

impl JavaSource {
    pub(super) fn new(label: String, source: String) -> Self {
        Self { label, source }
    }
}

pub(super) fn type_names(source: &str) -> Vec<String> {
    let tokens = source_tokens(source);
    let package = package_name(&tokens);
    let mut names = Vec::new();
    let mut depth = 0usize;
    let mut i = 0usize;

    while i < tokens.len() {
        match tokens[i].kind {
            JavaSyntaxKind::LBrace => {
                depth += 1;
                i += 1;
            }
            JavaSyntaxKind::RBrace => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            kind if depth == 0 && is_type_keyword(kind) => {
                if let Some(simple_name) = next_ident(&tokens, i + 1) {
                    names.push(internal_name(package.as_deref(), simple_name));
                }
                i += 1;
            }
            JavaSyntaxKind::At
                if depth == 0
                    && tokens
                        .get(i + 1)
                        .is_some_and(|token| token.kind == JavaSyntaxKind::InterfaceKw) =>
            {
                if let Some(simple_name) = next_ident(&tokens, i + 2) {
                    names.push(internal_name(package.as_deref(), simple_name));
                }
                i += 3;
            }
            _ => i += 1,
        }
    }

    names
}

pub(super) fn register_members(
    catalog: &mut ClassCatalog,
    errors: &mut Vec<String>,
    sources: &[JavaSource],
) {
    for source in sources {
        register_source_members(catalog, errors, &source.label, &source.source);
    }
}

fn register_source_members(
    catalog: &mut ClassCatalog,
    errors: &mut Vec<String>,
    label: &str,
    source: &str,
) {
    let parse = crate::parser::Parser::parse(source);
    if !parse.errors.is_empty() {
        errors.push(format!("failed to parse classpath source {label}"));
        return;
    }

    let root = JavaSyntaxNode::new_root(parse.green_node);
    let Some(unit) = AstCompilationUnit::cast(root) else {
        return;
    };
    let tokens = source_tokens(source);
    let package = package_name(&tokens);
    let imports = SourceImports::from_tokens(&tokens);

    for decl in unit.type_decls() {
        match decl {
            AstTypeDecl::Class(class) => {
                register_source_class(catalog, class, package.as_deref(), &imports, false)
            }
            AstTypeDecl::Interface(interface) => {
                register_source_interface(catalog, interface, package.as_deref(), &imports)
            }
            _ => {}
        }
    }
}

fn register_source_class(
    catalog: &mut ClassCatalog,
    class: AstClassDecl,
    package: Option<&str>,
    imports: &SourceImports,
    is_interface: bool,
) {
    let Some(name) = class.name() else {
        return;
    };
    let owner = internal_name(package, name.text());
    catalog.insert_internal_class(&owner);
    if is_interface {
        catalog.mark_interface(&owner);
    }
    let type_vars = source_type_params(class.syntax());
    let resolver = SourceTypeResolver {
        catalog: catalog.clone(),
        package,
        imports,
        type_vars: type_vars.clone(),
    };
    register_declared_parents(catalog, &owner, class.syntax(), &resolver);
    let Some(body) = class.body() else {
        return;
    };
    register_member_nodes(
        catalog,
        &owner,
        body.syntax(),
        package,
        imports,
        is_interface,
        type_vars,
    );
}

fn register_source_interface(
    catalog: &mut ClassCatalog,
    interface: AstInterfaceDecl,
    package: Option<&str>,
    imports: &SourceImports,
) {
    let Some(name) = interface.name() else {
        return;
    };
    let owner = internal_name(package, name.text());
    catalog.insert_internal_class(&owner);
    catalog.mark_interface(&owner);
    let type_vars = source_type_params(interface.syntax());
    let resolver = SourceTypeResolver {
        catalog: catalog.clone(),
        package,
        imports,
        type_vars: type_vars.clone(),
    };
    register_declared_parents(catalog, &owner, interface.syntax(), &resolver);
    let Some(body) = interface.body() else {
        return;
    };
    register_member_nodes(
        catalog,
        &owner,
        body.syntax(),
        package,
        imports,
        true,
        type_vars,
    );
}

fn register_member_nodes(
    catalog: &mut ClassCatalog,
    owner: &str,
    body: &JavaSyntaxNode,
    package: Option<&str>,
    imports: &SourceImports,
    is_interface: bool,
    type_vars: HashSet<String>,
) {
    let resolver = SourceTypeResolver {
        catalog: catalog.clone(),
        package,
        imports,
        type_vars,
    };
    let mut pending_flags = 0;

    for child in body.children() {
        match child.kind() {
            JavaSyntaxKind::ModifierList => pending_flags = source_access_flags(&child),
            JavaSyntaxKind::FieldDecl => {
                if let Some(field) = AstFieldDecl::cast(child) {
                    let flags = if is_interface {
                        pending_flags | ACC_PUBLIC | ACC_STATIC | ACC_FINAL
                    } else {
                        pending_flags
                    };
                    register_source_field(catalog, owner, field, flags, &resolver);
                }
                pending_flags = 0;
            }
            JavaSyntaxKind::MethodDecl => {
                if let Some(method) = AstMethodDecl::cast(child) {
                    register_source_method(
                        catalog,
                        owner,
                        method,
                        pending_flags,
                        &resolver,
                        is_interface,
                    );
                }
                pending_flags = 0;
            }
            JavaSyntaxKind::ConstructorDecl => pending_flags = 0,
            JavaSyntaxKind::ClassDecl
            | JavaSyntaxKind::InterfaceDecl
            | JavaSyntaxKind::EnumDecl
            | JavaSyntaxKind::RecordDecl => pending_flags = 0,
            _ => {}
        }
    }
}

fn register_source_field(
    catalog: &mut ClassCatalog,
    owner: &str,
    field: AstFieldDecl,
    access_flags: u16,
    resolver: &SourceTypeResolver<'_>,
) {
    let Some(ty) = field.ty().and_then(|ty| resolver.resolve_type(ty.syntax())) else {
        return;
    };

    for declarator in field
        .syntax()
        .descendants()
        .filter(|node| node.kind() == JavaSyntaxKind::VarDeclarator)
    {
        if let Some(name) = first_ident_text(&declarator) {
            catalog.insert_field(
                owner,
                name,
                ty.erasure().descriptor(),
                ty.clone(),
                access_flags,
            );
        }
    }
}

fn register_source_method(
    catalog: &mut ClassCatalog,
    owner: &str,
    method: AstMethodDecl,
    access_flags: u16,
    resolver: &SourceTypeResolver<'_>,
    is_interface: bool,
) {
    let Some(name) = method.name() else {
        return;
    };
    let return_ty = method
        .return_type()
        .and_then(|ty| resolver.resolve_type(ty.syntax()))
        .unwrap_or(Ty::Void);
    let Some(params) = source_method_params(method.syntax(), resolver) else {
        return;
    };
    let sig = MethodSig::new(Ustr::from(name.text()), params, return_ty);
    let mut flags = if is_interface {
        access_flags | ACC_PUBLIC | ACC_ABSTRACT
    } else {
        access_flags
    };
    if has_token(method.syntax(), JavaSyntaxKind::Ellipsis) {
        flags |= ACC_VARARGS;
    }
    catalog.insert_method(owner, sig, flags, is_interface);
}

fn register_declared_parents(
    catalog: &mut ClassCatalog,
    owner: &str,
    node: &JavaSyntaxNode,
    resolver: &SourceTypeResolver<'_>,
) {
    for parent in declared_parent_types(node, resolver) {
        catalog.insert_parent(owner, parent.internal_name());
    }
}

#[derive(Debug, Clone)]
struct Token {
    kind: JavaSyntaxKind,
    text: String,
}

#[derive(Debug, Clone, Default)]
struct SourceImports {
    exact: HashMap<String, String>,
    wildcard: Vec<String>,
}

impl SourceImports {
    fn from_tokens(tokens: &[Token]) -> Self {
        let mut imports = Self::default();
        let mut i = 0usize;

        while i < tokens.len() {
            if tokens[i].kind != JavaSyntaxKind::ImportKw {
                i += 1;
                continue;
            }

            i += 1;
            if tokens
                .get(i)
                .is_some_and(|token| token.kind == JavaSyntaxKind::StaticKw)
            {
                i = skip_until_semi(tokens, i);
                continue;
            }

            let (path, next, wildcard) = import_path(tokens, i);
            if let Some(path) = path {
                let internal = path.replace('.', "/");
                if wildcard {
                    imports.wildcard.push(internal);
                } else if let Some(simple) = path.rsplit('.').next() {
                    imports.exact.insert(simple.to_string(), internal);
                }
            }
            i = skip_until_semi(tokens, next);
        }

        imports
    }
}

#[derive(Clone)]
struct SourceTypeResolver<'a> {
    catalog: ClassCatalog,
    package: Option<&'a str>,
    imports: &'a SourceImports,
    type_vars: HashSet<String>,
}

impl SourceTypeResolver<'_> {
    fn resolve_type(&self, node: &JavaSyntaxNode) -> Option<Ty> {
        let tokens = node_tokens(node);
        let mut ty = self.resolve_base_type(&tokens)?;

        for _ in 0..source_array_dimensions(&tokens) {
            ty = Ty::Array(Box::new(ty));
        }
        if tokens
            .iter()
            .any(|token| token.kind == JavaSyntaxKind::Ellipsis)
        {
            ty = Ty::Array(Box::new(ty));
        }

        Some(ty)
    }

    fn resolve_base_type(&self, tokens: &[Token]) -> Option<Ty> {
        let index = tokens.iter().position(|token| is_type_token(token.kind))?;
        match tokens[index].kind {
            JavaSyntaxKind::VoidKw => Some(Ty::Void),
            JavaSyntaxKind::BooleanKw => Some(Ty::Boolean),
            JavaSyntaxKind::ByteKw => Some(Ty::Byte),
            JavaSyntaxKind::CharKw => Some(Ty::Char),
            JavaSyntaxKind::ShortKw => Some(Ty::Short),
            JavaSyntaxKind::IntKw => Some(Ty::Int),
            JavaSyntaxKind::LongKw => Some(Ty::Long),
            JavaSyntaxKind::FloatKw => Some(Ty::Float),
            JavaSyntaxKind::DoubleKw => Some(Ty::Double),
            JavaSyntaxKind::Ident => {
                let name = type_name_from_tokens(tokens, index)?;
                self.resolve_named_type(&name)
            }
            _ => None,
        }
    }

    fn resolve_named_type(&self, name: &str) -> Option<Ty> {
        if self.type_vars.contains(name) {
            return Some(Ty::object());
        }

        if name.contains('.') {
            return self
                .catalog
                .resolve_qualified_name(name)
                .map(|internal| Ty::class(Ustr::from(internal)));
        }

        if let Some(internal) = self.imports.exact.get(name) {
            return Some(Ty::class(Ustr::from(internal.as_str())));
        }

        if let Some(internal) = self.catalog.resolve_java_lang(name) {
            return Some(Ty::class(Ustr::from(internal)));
        }

        if let Some(package) = self.package {
            let internal = format!("{}/{}", package.replace('.', "/"), name);
            if self.catalog.contains_internal_class(&internal) {
                return Some(Ty::class(Ustr::from(&internal)));
            }
        }

        for package in &self.imports.wildcard {
            let internal = format!("{package}/{name}");
            if self.catalog.contains_internal_class(&internal) {
                return Some(Ty::class(Ustr::from(&internal)));
            }
        }

        self.catalog
            .resolve_simple_name(name)
            .map(|internal| Ty::class(Ustr::from(internal)))
    }
}

fn source_tokens(source: &str) -> Vec<Token> {
    Lexer::new(source)
        .map(|token| Token {
            kind: token.kind,
            text: token.text,
        })
        .collect()
}

fn node_tokens(node: &JavaSyntaxNode) -> Vec<Token> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .map(|token| Token {
            kind: token.kind(),
            text: token.text().to_string(),
        })
        .collect()
}

fn import_path(tokens: &[Token], start: usize) -> (Option<String>, usize, bool) {
    let mut parts = Vec::new();
    let mut i = start;
    let mut wildcard = false;

    while let Some(token) = tokens.get(i) {
        match token.kind {
            JavaSyntaxKind::Ident => {
                parts.push(token.text.clone());
                i += 1;
            }
            JavaSyntaxKind::Dot => {
                if tokens
                    .get(i + 1)
                    .is_some_and(|next| next.kind == JavaSyntaxKind::Star)
                {
                    wildcard = true;
                    i += 2;
                    break;
                }
                i += 1;
            }
            _ => break,
        }
    }

    ((!parts.is_empty()).then_some(parts.join(".")), i, wildcard)
}

fn skip_until_semi(tokens: &[Token], start: usize) -> usize {
    tokens
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, token)| (token.kind == JavaSyntaxKind::Semi).then_some(index + 1))
        .unwrap_or(tokens.len())
}

fn package_name(tokens: &[Token]) -> Option<String> {
    let package_index = tokens
        .iter()
        .position(|token| token.kind == JavaSyntaxKind::PackageKw)?;
    let (package, _) = qualified_name(tokens, package_index + 1)?;
    Some(package)
}

fn qualified_name(tokens: &[Token], start: usize) -> Option<(String, usize)> {
    let mut name = String::new();
    let mut i = start;
    let mut expecting_ident = true;

    while let Some(token) = tokens.get(i) {
        match token.kind {
            JavaSyntaxKind::Ident if expecting_ident => {
                name.push_str(&token.text);
                expecting_ident = false;
            }
            JavaSyntaxKind::Dot if !expecting_ident => {
                name.push('.');
                expecting_ident = true;
            }
            _ => break,
        }
        i += 1;
    }

    (!name.is_empty() && !expecting_ident).then_some((name, i))
}

fn next_ident(tokens: &[Token], start: usize) -> Option<&str> {
    tokens
        .iter()
        .skip(start)
        .find(|token| token.kind == JavaSyntaxKind::Ident)
        .map(|token| token.text.as_str())
}

fn is_type_keyword(kind: JavaSyntaxKind) -> bool {
    matches!(
        kind,
        JavaSyntaxKind::ClassKw
            | JavaSyntaxKind::InterfaceKw
            | JavaSyntaxKind::EnumKw
            | JavaSyntaxKind::RecordKw
    )
}

fn internal_name(package: Option<&str>, simple_name: &str) -> String {
    match package {
        Some(package) => format!("{}/{}", package.replace('.', "/"), simple_name),
        None => simple_name.to_string(),
    }
}

fn source_method_params(
    method: &JavaSyntaxNode,
    resolver: &SourceTypeResolver<'_>,
) -> Option<Vec<Ty>> {
    let params = method
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::FormalParamList)?;

    params
        .children()
        .filter(|child| child.kind() == JavaSyntaxKind::FormalParam)
        .map(|param| {
            let mut ty = param
                .children()
                .find(|child| child.kind() == JavaSyntaxKind::Type)
                .and_then(|ty| resolver.resolve_type(&ty))?;
            if has_token(&param, JavaSyntaxKind::Ellipsis) {
                ty = Ty::Array(Box::new(ty));
            }
            Some(ty)
        })
        .collect()
}

fn declared_parent_types(node: &JavaSyntaxNode, resolver: &SourceTypeResolver<'_>) -> Vec<Ty> {
    let tokens = node_tokens(node);
    let mut parents = Vec::new();
    let mut i = 0usize;

    while let Some(token) = tokens.get(i) {
        match token.kind {
            JavaSyntaxKind::LBrace => break,
            JavaSyntaxKind::ExtendsKw | JavaSyntaxKind::ImplementsKw => {
                i += 1;
                while let Some(token) = tokens.get(i) {
                    if matches!(
                        token.kind,
                        JavaSyntaxKind::LBrace | JavaSyntaxKind::PermitsKw
                    ) {
                        break;
                    }
                    if token.kind == JavaSyntaxKind::Comma {
                        i += 1;
                        continue;
                    }
                    if let Some(name) = type_name_from_tokens(&tokens, i)
                        && let Some(ty) = resolver.resolve_named_type(&name)
                    {
                        parents.push(ty);
                    }
                    i = skip_parent_type(&tokens, i);
                }
            }
            _ => i += 1,
        }
    }

    parents
}

fn skip_parent_type(tokens: &[Token], start: usize) -> usize {
    let mut i = start;
    let mut generic_depth = 0usize;

    while let Some(token) = tokens.get(i) {
        match token.kind {
            JavaSyntaxKind::Lt => {
                generic_depth += 1;
                i += 1;
            }
            JavaSyntaxKind::Gt if generic_depth > 0 => {
                generic_depth -= 1;
                i += 1;
            }
            JavaSyntaxKind::Comma
            | JavaSyntaxKind::LBrace
            | JavaSyntaxKind::ImplementsKw
            | JavaSyntaxKind::PermitsKw
                if generic_depth == 0 =>
            {
                break;
            }
            _ => i += 1,
        }
    }

    i
}

fn first_ident_text(node: &JavaSyntaxNode) -> Option<String> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == JavaSyntaxKind::Ident)
        .map(|token| token.text().to_string())
}

fn has_token(node: &JavaSyntaxNode, kind: JavaSyntaxKind) -> bool {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == kind)
}

fn source_type_params(node: &JavaSyntaxNode) -> HashSet<String> {
    let tokens = node_tokens(node);
    let mut params = HashSet::new();
    let mut i = 0usize;

    while i < tokens.len() {
        if tokens[i].kind == JavaSyntaxKind::LBrace {
            break;
        }
        if tokens[i].kind != JavaSyntaxKind::Lt {
            i += 1;
            continue;
        }

        i += 1;
        let mut expecting_name = true;
        while let Some(token) = tokens.get(i) {
            match token.kind {
                JavaSyntaxKind::Ident if expecting_name => {
                    params.insert(token.text.clone());
                    expecting_name = false;
                    i += 1;
                }
                JavaSyntaxKind::Comma => {
                    expecting_name = true;
                    i += 1;
                }
                JavaSyntaxKind::ExtendsKw => {
                    while let Some(token) = tokens.get(i) {
                        if matches!(token.kind, JavaSyntaxKind::Comma | JavaSyntaxKind::Gt) {
                            break;
                        }
                        i += 1;
                    }
                }
                JavaSyntaxKind::Gt => {
                    i += 1;
                    break;
                }
                _ => i += 1,
            }
        }
    }

    params
}

fn source_access_flags(node: &JavaSyntaxNode) -> u16 {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .fold(0, |flags, token| {
            flags
                | match token.kind() {
                    JavaSyntaxKind::PublicKw => ACC_PUBLIC,
                    JavaSyntaxKind::PrivateKw => ACC_PRIVATE,
                    JavaSyntaxKind::ProtectedKw => ACC_PROTECTED,
                    JavaSyntaxKind::StaticKw => ACC_STATIC,
                    JavaSyntaxKind::FinalKw => ACC_FINAL,
                    JavaSyntaxKind::AbstractKw => ACC_ABSTRACT,
                    _ => 0,
                }
        })
}

fn source_array_dimensions(tokens: &[Token]) -> usize {
    tokens
        .windows(2)
        .filter(|window| {
            window[0].kind == JavaSyntaxKind::LBrack && window[1].kind == JavaSyntaxKind::RBrack
        })
        .count()
}

fn type_name_from_tokens(tokens: &[Token], start: usize) -> Option<String> {
    let mut name = String::new();
    let mut expecting_ident = true;
    let mut i = start;

    while let Some(token) = tokens.get(i) {
        match token.kind {
            JavaSyntaxKind::Ident if expecting_ident => {
                name.push_str(&token.text);
                expecting_ident = false;
            }
            JavaSyntaxKind::Dot if !expecting_ident => {
                name.push('.');
                expecting_ident = true;
            }
            JavaSyntaxKind::Lt | JavaSyntaxKind::LBrack | JavaSyntaxKind::Ellipsis => break,
            _ if !name.is_empty() => break,
            _ => {}
        }
        i += 1;
    }

    (!name.is_empty() && !expecting_ident).then_some(name)
}

fn is_type_token(kind: JavaSyntaxKind) -> bool {
    matches!(
        kind,
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
    )
}
