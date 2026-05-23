use crate::hir::{Import, Package};
use crate::lowering::syntax::token_source_line;
use crate::lowering::{LowerError, LowerResult};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode, JavaSyntaxToken};
use javac_ty::Ty;
use std::collections::{HashMap, HashSet};
use ustr::Ustr;

#[derive(Debug, Clone, Default)]
pub(super) struct TypeResolver {
    exact_imports: HashMap<String, String>,
    wildcard_imports: Vec<String>,
    current_class: Option<String>,
}

impl TypeResolver {
    pub fn for_class(
        package: Option<&Package>,
        imports: &[Import],
        current_class: &str,
    ) -> LowerResult<Self> {
        let mut resolver = Self {
            exact_imports: HashMap::new(),
            wildcard_imports: Vec::new(),
            current_class: Some(current_class.to_string()),
        };

        for import in imports {
            resolver.add_import(import)?;
        }
        if let Some(package) = package {
            resolver
                .wildcard_imports
                .push(package.name.as_str().replace('.', "/"));
        }

        Ok(resolver)
    }

    pub fn resolve_type_name(
        &self,
        name: &str,
        line: u16,
        type_vars: &HashSet<Ustr>,
    ) -> LowerResult<Ty> {
        if !name.contains('.') && type_vars.contains(&Ustr::from(name)) {
            return Ok(Ty::TypeVar(Ustr::from(name)));
        }

        if let Some(current_class) = self.current_class_name(name) {
            return Ok(Ty::Class(Ustr::from(current_class)));
        }

        if name.contains('.') {
            return self.resolve_qualified_type(name, line);
        }

        if let Some(internal) = self.exact_imports.get(name) {
            return Ok(Ty::Class(Ustr::from(internal.as_str())));
        }

        if let Some(internal) = javac_call_resolver::resolve_class_name(name)
            && internal.starts_with("java/lang/")
        {
            return Ok(Ty::Class(Ustr::from(internal)));
        }

        for package in &self.wildcard_imports {
            let internal = format!("{package}/{name}");
            if javac_call_resolver::resolve_internal_class_name(&internal).is_some() {
                return Ok(Ty::Class(Ustr::from(&internal)));
            }
        }

        Err(LowerError::UnknownType {
            name: name.to_string(),
            line,
        })
    }

    fn add_import(&mut self, import: &Import) -> LowerResult<()> {
        let line = import.source_line.unwrap_or(1);
        if import.is_static {
            return Ok(());
        }

        let path = import.path.as_str();
        if javac_call_resolver::resolve_import(path, import.is_wildcard).is_none() {
            return Err(LowerError::UnknownImport {
                name: path.to_string(),
                line,
            });
        }

        let internal = path.replace('.', "/");
        if import.is_wildcard {
            self.wildcard_imports.push(internal);
        } else if let Some(simple) = path.rsplit('.').next() {
            self.exact_imports.insert(simple.to_string(), internal);
        }
        Ok(())
    }

    fn current_class_name(&self, name: &str) -> Option<&str> {
        let current_class = self.current_class.as_deref()?;
        let simple_name = current_class.rsplit('/').next().unwrap_or(current_class);
        (name == simple_name).then_some(current_class)
    }

    fn resolve_qualified_type(&self, name: &str, line: u16) -> LowerResult<Ty> {
        let internal = name.replace('.', "/");
        if javac_call_resolver::resolve_internal_class_name(&internal).is_some() {
            return Ok(Ty::Class(Ustr::from(&internal)));
        }

        Err(LowerError::UnknownType {
            name: name.to_string(),
            line,
        })
    }
}

pub(super) fn lower_type(node: &JavaSyntaxNode, resolver: &TypeResolver) -> LowerResult<Ty> {
    lower_type_with_vars(node, &HashSet::new(), resolver)
}

pub(super) fn lower_type_with_vars(
    node: &JavaSyntaxNode,
    type_vars: &HashSet<Ustr>,
    resolver: &TypeResolver,
) -> LowerResult<Ty> {
    let mut base = lower_base_type(node, type_vars, resolver)?;
    for _ in 0..array_dimensions(node) {
        base = Ty::Array(Box::new(base));
    }
    Ok(base)
}

pub(super) fn is_var_type(node: &JavaSyntaxNode) -> bool {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == JavaSyntaxKind::VarKw)
}

pub(super) fn is_string_ty(ty: &Ty) -> bool {
    matches!(ty, Ty::Class(name) if name.as_str() == "java/lang/String")
}

pub(super) fn class_type_from_name(
    name: &str,
    line: u16,
    resolver: &TypeResolver,
) -> LowerResult<Ty> {
    resolver.resolve_type_name(name, line, &HashSet::new())
}

fn lower_base_type(
    node: &JavaSyntaxNode,
    type_vars: &HashSet<Ustr>,
    resolver: &TypeResolver,
) -> LowerResult<Ty> {
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
        JavaSyntaxKind::Ident => {
            let line = token_source_line(&token);
            let name = type_name_text(node).unwrap_or_else(|| token.text().to_string());
            resolver.resolve_type_name(&name, line, type_vars)?
        }
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

fn array_dimensions(node: &JavaSyntaxNode) -> usize {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == JavaSyntaxKind::LBrack)
        .count()
}

fn type_name_text(node: &JavaSyntaxNode) -> Option<String> {
    let mut text = String::new();
    let mut has_ident = false;

    for token in node
        .descendants_with_tokens()
        .filter_map(|it| it.into_token())
    {
        match token.kind() {
            JavaSyntaxKind::Ident => {
                text.push_str(token.text());
                has_ident = true;
            }
            JavaSyntaxKind::Dot if has_ident => text.push('.'),
            JavaSyntaxKind::Lt | JavaSyntaxKind::LBrack => break,
            JavaSyntaxKind::Whitespace | JavaSyntaxKind::Comment => {}
            _ if has_ident => break,
            _ => {}
        }
    }

    has_ident.then_some(text)
}
