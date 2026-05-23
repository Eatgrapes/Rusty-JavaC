use crate::lowering::syntax::source_line;
use crate::lowering::types::{TypeResolver, lower_type};
use crate::lowering::{LowerError, LowerResult};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode, JavaSyntaxToken};
use javac_ty::{Ty, TypeParam};
use std::collections::HashSet;
use ustr::Ustr;

pub(super) fn lower_type_params(
    owner: &JavaSyntaxNode,
    resolver: &TypeResolver,
) -> LowerResult<Vec<TypeParam>> {
    let Some(list) = owner
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::TypeParamList)
    else {
        return Ok(Vec::new());
    };

    list.children()
        .filter(|child| child.kind() == JavaSyntaxKind::TypeParam)
        .map(|param| lower_type_param(param, resolver))
        .collect()
}

pub(super) fn class_signature(
    class: &JavaSyntaxNode,
    type_params: &[TypeParam],
    resolver: &TypeResolver,
) -> LowerResult<Option<String>> {
    if type_params.is_empty() && !has_generic_type(class) {
        return Ok(None);
    }

    let vars = type_var_names(type_params);
    let mut signature = type_params_signature(type_params);
    let super_type = class
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::ExtendsClause)
        .and_then(|extends| type_children(&extends).next())
        .map(|ty| type_signature(&ty, &vars, resolver))
        .transpose()?
        .unwrap_or_else(|| "Ljava/lang/Object;".to_string());
    signature.push_str(&super_type);

    let interfaces = class
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::ImplementsClause)
        .into_iter()
        .flat_map(|implements| type_children(&implements).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for interface in interfaces {
        signature.push_str(&type_signature(&interface, &vars, resolver)?);
    }

    Ok(Some(signature))
}

pub(super) fn method_signature(
    method: &JavaSyntaxNode,
    class_type_params: &[TypeParam],
    method_type_params: &[TypeParam],
    resolver: &TypeResolver,
) -> LowerResult<Option<String>> {
    let mut all_params = class_type_params.to_vec();
    all_params.extend_from_slice(method_type_params);
    let vars = type_var_names(&all_params);
    if method_type_params.is_empty() && !has_generic_type(method) && !uses_type_var(method, &vars) {
        return Ok(None);
    }

    let mut signature = type_params_signature(method_type_params);
    signature.push('(');
    if let Some(params) = method
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::FormalParamList)
    {
        for param in params
            .children()
            .filter(|child| child.kind() == JavaSyntaxKind::FormalParam)
        {
            let ty = param
                .children()
                .find(|child| child.kind() == JavaSyntaxKind::Type)
                .ok_or(LowerError::MissingType)?;
            signature.push_str(&type_signature(&ty, &vars, resolver)?);
        }
    }
    signature.push(')');

    let return_type = method
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Type)
        .ok_or(LowerError::MissingType)?;
    signature.push_str(&type_signature(&return_type, &vars, resolver)?);

    Ok(Some(signature))
}

fn lower_type_param(node: JavaSyntaxNode, resolver: &TypeResolver) -> LowerResult<TypeParam> {
    let name = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == JavaSyntaxKind::Ident)
        .ok_or(LowerError::MissingType)?;
    let bound_types = node
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::TypeBound)
        .into_iter()
        .flat_map(|bound| type_children(&bound).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let bounds = bound_types
        .into_iter()
        .map(|ty| lower_type(&ty, resolver))
        .collect::<LowerResult<Vec<_>>>()?;

    Ok(TypeParam {
        name: Ustr::from(name.text()),
        bounds,
    })
}

fn has_generic_type(node: &JavaSyntaxNode) -> bool {
    node.descendants().any(|child| {
        matches!(
            child.kind(),
            JavaSyntaxKind::TypeArgList | JavaSyntaxKind::TypeParamList
        )
    })
}

fn uses_type_var(node: &JavaSyntaxNode, type_vars: &HashSet<String>) -> bool {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == JavaSyntaxKind::Ident && type_vars.contains(token.text()))
}

fn type_children(node: &JavaSyntaxNode) -> impl Iterator<Item = JavaSyntaxNode> + '_ {
    node.children()
        .filter(|child| child.kind() == JavaSyntaxKind::Type)
}

fn type_var_names(type_params: &[TypeParam]) -> HashSet<String> {
    type_params
        .iter()
        .map(|param| param.name.to_string())
        .collect()
}

fn type_params_signature(type_params: &[TypeParam]) -> String {
    if type_params.is_empty() {
        return String::new();
    }

    let mut signature = String::from("<");
    for param in type_params {
        signature.push_str(param.name.as_str());
        if param.bounds.is_empty() {
            signature.push_str(":Ljava/lang/Object;");
        } else {
            for bound in &param.bounds {
                signature.push(':');
                signature.push_str(&ty_signature(bound));
            }
        }
    }
    signature.push('>');
    signature
}

fn type_signature(
    node: &JavaSyntaxNode,
    type_vars: &HashSet<String>,
    resolver: &TypeResolver,
) -> LowerResult<String> {
    let tokens = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(is_type_signature_token)
        .collect::<Vec<_>>();
    let mut parser = TypeSignatureParser {
        tokens: &tokens,
        pos: 0,
        type_vars,
        resolver,
        line: source_line(node),
    };
    parser.parse_type()
}

fn ty_signature(ty: &Ty) -> String {
    match ty {
        Ty::TypeVar(name) => format!("T{};", name.as_str()),
        Ty::Array(element) => format!("[{}", ty_signature(element)),
        other => other.descriptor(),
    }
}

fn is_type_signature_token(token: &JavaSyntaxToken) -> bool {
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
            | JavaSyntaxKind::Dot
            | JavaSyntaxKind::Lt
            | JavaSyntaxKind::Gt
            | JavaSyntaxKind::Comma
            | JavaSyntaxKind::Question
            | JavaSyntaxKind::ExtendsKw
            | JavaSyntaxKind::SuperKw
            | JavaSyntaxKind::LBrack
            | JavaSyntaxKind::RBrack
    )
}

struct TypeSignatureParser<'a> {
    tokens: &'a [JavaSyntaxToken],
    pos: usize,
    type_vars: &'a HashSet<String>,
    resolver: &'a TypeResolver,
    line: u16,
}

impl TypeSignatureParser<'_> {
    fn parse_type(&mut self) -> LowerResult<String> {
        let mut base = self.parse_base_type()?;
        while self.eat(JavaSyntaxKind::LBrack) {
            self.expect(JavaSyntaxKind::RBrack)?;
            base = format!("[{base}");
        }
        Ok(base)
    }

    fn parse_base_type(&mut self) -> LowerResult<String> {
        let Some(token) = self.peek().cloned() else {
            return Err(LowerError::MissingType);
        };

        match token.kind() {
            JavaSyntaxKind::VoidKw => {
                self.pos += 1;
                Ok("V".to_string())
            }
            JavaSyntaxKind::BooleanKw => self.primitive("Z"),
            JavaSyntaxKind::ByteKw => self.primitive("B"),
            JavaSyntaxKind::CharKw => self.primitive("C"),
            JavaSyntaxKind::ShortKw => self.primitive("S"),
            JavaSyntaxKind::IntKw => self.primitive("I"),
            JavaSyntaxKind::LongKw => self.primitive("J"),
            JavaSyntaxKind::FloatKw => self.primitive("F"),
            JavaSyntaxKind::DoubleKw => self.primitive("D"),
            JavaSyntaxKind::Ident => self.parse_class_or_type_var(),
            _ => Err(LowerError::MissingType),
        }
    }

    fn primitive(&mut self, descriptor: &str) -> LowerResult<String> {
        self.pos += 1;
        Ok(descriptor.to_string())
    }

    fn parse_class_or_type_var(&mut self) -> LowerResult<String> {
        let first = self.expect_ident()?;
        if self.peek().is_none() && self.type_vars.contains(&first) {
            return Ok(format!("T{first};"));
        }

        let mut source_name = first;
        let mut args = self.parse_type_args()?;
        while self.eat(JavaSyntaxKind::Dot) {
            source_name.push('.');
            source_name.push_str(&self.expect_ident()?);
            args.push_str(&self.parse_type_args()?);
        }

        let internal_name = self
            .resolver
            .resolve_type_name(&source_name, self.line, &HashSet::new())?
            .internal_name();
        Ok(format!("L{internal_name}{args};"))
    }

    fn parse_type_args(&mut self) -> LowerResult<String> {
        if !self.eat(JavaSyntaxKind::Lt) {
            return Ok(String::new());
        }

        let mut args = String::from("<");
        while !self.eat(JavaSyntaxKind::Gt) {
            if self.eat(JavaSyntaxKind::Question) {
                if self.eat(JavaSyntaxKind::ExtendsKw) {
                    args.push('+');
                    args.push_str(&self.parse_type()?);
                } else if self.eat(JavaSyntaxKind::SuperKw) {
                    args.push('-');
                    args.push_str(&self.parse_type()?);
                } else {
                    args.push('*');
                }
            } else {
                args.push_str(&self.parse_type()?);
            }
            self.eat(JavaSyntaxKind::Comma);
        }
        args.push('>');
        Ok(args)
    }

    fn peek(&self) -> Option<&JavaSyntaxToken> {
        self.tokens.get(self.pos)
    }

    fn eat(&mut self, kind: JavaSyntaxKind) -> bool {
        if self.peek().is_some_and(|token| token.kind() == kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: JavaSyntaxKind) -> LowerResult<()> {
        if self.eat(kind) {
            Ok(())
        } else {
            Err(LowerError::MissingType)
        }
    }

    fn expect_ident(&mut self) -> LowerResult<String> {
        let Some(token) = self.peek() else {
            return Err(LowerError::MissingType);
        };
        if token.kind() != JavaSyntaxKind::Ident {
            return Err(LowerError::MissingType);
        }
        let text = token.text().to_string();
        self.pos += 1;
        Ok(text)
    }
}
