use crate::hir::*;
use javac_ast::ast::{
    AstNode, ClassBody, ClassDecl, CompilationUnit as AstCompilationUnit,
    ImportDecl as AstImportDecl, MethodDecl as AstMethodDecl,
};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode, JavaSyntaxToken};
use javac_ty::{MethodSig, Ty};
use std::collections::HashMap;
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
    UnsupportedExpression,
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
            LowerError::UnsupportedExpression => "unsupported expression",
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
    let mut body_builder = BodyBuilder::default();
    let root_block = lower_method_body(access_flags, &method, &mut body_builder)?;

    Ok(MethodDecl {
        id: HirId(method_index + 1),
        name: Ustr::from(name.text()),
        signature,
        access_flags,
        body: body_builder.body,
        root_block,
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

fn lower_method_body(
    access_flags: u16,
    method: &AstMethodDecl,
    body: &mut BodyBuilder,
) -> LowerResult<Option<Block>> {
    let has_code = access_flags & (ACC_ABSTRACT | ACC_NATIVE) == 0;
    if has_code && let Some(method_body) = method.body() {
        method_body
            .block()
            .map(|block| lower_block(block.syntax(), body).map(Some))
            .unwrap_or(Ok(Some(Block { stmts: Vec::new() })))
    } else {
        Ok(None)
    }
}

fn lower_block(block: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Block> {
    let mut stmts = Vec::new();
    for child in block.children() {
        match child.kind() {
            JavaSyntaxKind::LocalVarDecl => stmts.extend(lower_local_var_decl(&child, body)?),
            JavaSyntaxKind::ExprStmt => {
                if let Some(stmt) = lower_expr_stmt(&child, body)? {
                    stmts.push(stmt);
                }
            }
            JavaSyntaxKind::Block => {
                let nested = lower_block(&child, body)?;
                stmts.push(body.alloc_stmt(Stmt::Block(nested)));
            }
            _ => {}
        }
    }
    Ok(Block { stmts })
}

fn lower_local_var_decl(decl: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Vec<StmtId>> {
    let declared_ty = decl
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Type)
        .ok_or(LowerError::MissingType)?;
    let explicit_ty = lower_type(&declared_ty)?;
    let is_var = is_var_type(&declared_ty);
    let mut stmts = Vec::new();

    for declarator in decl
        .descendants()
        .filter(|node| node.kind() == JavaSyntaxKind::VarDeclarator)
    {
        let name = first_ident(&declarator).ok_or(LowerError::MissingMethodName)?;
        let initializer = if let Some(tokens) = initializer_tokens(&declarator) {
            body.lower_expr_tokens(&tokens)?
        } else {
            None
        };
        let ty = if is_var {
            initializer
                .map(|expr| body.expr_ty(expr))
                .unwrap_or_else(|| Ty::Class(Ustr::from("java/lang/Object")))
        } else {
            explicit_ty.clone()
        };
        body.local_types.insert(Ustr::from(name.text()), ty.clone());
        stmts.push(body.alloc_stmt(Stmt::LocalVar(LocalVarDecl {
            ty,
            name: Ustr::from(name.text()),
            initializer,
        })));
    }

    Ok(stmts)
}

fn lower_expr_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Option<StmtId>> {
    let tokens = expr_tokens(stmt);
    if tokens.is_empty() {
        return Ok(None);
    }

    let expr = body
        .lower_expr_tokens(&tokens)?
        .ok_or(LowerError::UnsupportedExpression)?;
    Ok(Some(body.alloc_stmt(Stmt::Expr(expr))))
}

#[derive(Default)]
struct BodyBuilder {
    body: Body,
    local_types: HashMap<Ustr, Ty>,
}

impl BodyBuilder {
    fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        self.body.exprs.alloc(expr)
    }

    fn alloc_stmt(&mut self, stmt: Stmt) -> StmtId {
        self.body.stmts.alloc(stmt)
    }

    fn lower_expr_tokens(&mut self, tokens: &[ExprToken]) -> LowerResult<Option<ExprId>> {
        if tokens.is_empty() {
            return Ok(None);
        }

        let mut parser = ExprLowerer {
            tokens,
            pos: 0,
            body: self,
        };
        parser.parse_expr().map(Some)
    }

    fn expr_ty(&self, expr_id: ExprId) -> Ty {
        match &self.body.exprs[expr_id] {
            Expr::IntLiteral(_) => Ty::Int,
            Expr::LongLiteral(_) => Ty::Long,
            Expr::FloatLiteral(_) => Ty::Float,
            Expr::DoubleLiteral(_) => Ty::Double,
            Expr::BoolLiteral(_) => Ty::Boolean,
            Expr::CharLiteral(_) => Ty::Char,
            Expr::StringLiteral(_) => Ty::Class(Ustr::from("java/lang/String")),
            Expr::Ident(name) => self.local_types.get(name).cloned().unwrap_or(Ty::Int),
            Expr::Binary { op, left, right } if *op == BinaryOp::Add => {
                let left_ty = self.expr_ty(*left);
                let right_ty = self.expr_ty(*right);
                if is_string_ty(&left_ty) || is_string_ty(&right_ty) {
                    Ty::Class(Ustr::from("java/lang/String"))
                } else {
                    left_ty
                }
            }
            Expr::Parens(inner) => self.expr_ty(*inner),
            _ => Ty::Int,
        }
    }
}

struct ExprLowerer<'a, 'b> {
    tokens: &'a [ExprToken],
    pos: usize,
    body: &'b mut BodyBuilder,
}

impl ExprLowerer<'_, '_> {
    fn parse_expr(&mut self) -> LowerResult<ExprId> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> LowerResult<ExprId> {
        let mut left = self.parse_primary()?;
        while self.eat(JavaSyntaxKind::Plus) {
            let right = self.parse_primary()?;
            left = self.body.alloc_expr(Expr::Binary {
                op: BinaryOp::Add,
                left,
                right,
            });
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> LowerResult<ExprId> {
        let Some(token) = self.peek().cloned() else {
            return Err(LowerError::UnsupportedExpression);
        };

        match token.kind {
            JavaSyntaxKind::IntLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::IntLiteral(parse_int_literal(&token.text))))
            }
            JavaSyntaxKind::LongLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::LongLiteral(parse_long_literal(&token.text))))
            }
            JavaSyntaxKind::StringLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::StringLiteral(Ustr::from(&string_literal_value(
                        &token.text,
                    )))))
            }
            JavaSyntaxKind::TrueKw | JavaSyntaxKind::FalseKw => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::BoolLiteral(token.kind == JavaSyntaxKind::TrueKw)))
            }
            JavaSyntaxKind::NullKw => {
                self.pos += 1;
                Ok(self.body.alloc_expr(Expr::NullLiteral))
            }
            JavaSyntaxKind::ThisKw => {
                self.pos += 1;
                Ok(self.body.alloc_expr(Expr::This))
            }
            JavaSyntaxKind::Ident => self.parse_name_or_call(),
            JavaSyntaxKind::LParen => {
                self.pos += 1;
                let inner = self.parse_expr()?;
                self.expect(JavaSyntaxKind::RParen)?;
                Ok(self.body.alloc_expr(Expr::Parens(inner)))
            }
            _ => Err(LowerError::UnsupportedExpression),
        }
    }

    fn parse_name_or_call(&mut self) -> LowerResult<ExprId> {
        let mut segments = vec![self.expect_ident()?];
        while self.eat(JavaSyntaxKind::Dot) {
            segments.push(self.expect_ident()?);
        }

        if self.eat(JavaSyntaxKind::LParen) {
            let args = self.parse_args()?;
            let method = segments.pop().ok_or(LowerError::UnsupportedExpression)?;
            let target = if segments.is_empty() {
                None
            } else {
                Some(self.build_path_expr(&segments))
            };
            return Ok(self.body.alloc_expr(Expr::MethodCall {
                target,
                method: Ustr::from(&method),
                args,
            }));
        }

        Ok(self.build_path_expr(&segments))
    }

    fn parse_args(&mut self) -> LowerResult<Vec<ExprId>> {
        let mut args = Vec::new();
        if self.eat(JavaSyntaxKind::RParen) {
            return Ok(args);
        }

        loop {
            args.push(self.parse_expr()?);
            if self.eat(JavaSyntaxKind::Comma) {
                continue;
            }
            self.expect(JavaSyntaxKind::RParen)?;
            break;
        }

        Ok(args)
    }

    fn build_path_expr(&mut self, segments: &[String]) -> ExprId {
        let mut expr = self.body.alloc_expr(Expr::Ident(Ustr::from(&segments[0])));
        for segment in &segments[1..] {
            expr = self.body.alloc_expr(Expr::FieldAccess {
                target: expr,
                field: Ustr::from(segment),
            });
        }
        expr
    }

    fn peek(&self) -> Option<&ExprToken> {
        self.tokens.get(self.pos)
    }

    fn eat(&mut self, kind: JavaSyntaxKind) -> bool {
        if self.peek().is_some_and(|token| token.kind == kind) {
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
            Err(LowerError::UnsupportedExpression)
        }
    }

    fn expect_ident(&mut self) -> LowerResult<String> {
        let Some(token) = self.peek().cloned() else {
            return Err(LowerError::UnsupportedExpression);
        };
        if token.kind != JavaSyntaxKind::Ident {
            return Err(LowerError::UnsupportedExpression);
        }
        self.pos += 1;
        Ok(token.text)
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

fn is_var_type(node: &JavaSyntaxNode) -> bool {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == JavaSyntaxKind::VarKw)
}

fn first_ident(node: &JavaSyntaxNode) -> Option<JavaSyntaxToken> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == JavaSyntaxKind::Ident)
}

fn initializer_tokens(node: &JavaSyntaxNode) -> Option<Vec<ExprToken>> {
    let mut seen_eq = false;
    let tokens = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter_map(|token| {
            if token.kind() == JavaSyntaxKind::Eq {
                seen_eq = true;
                return None;
            }
            if seen_eq && is_expr_token(token.kind()) {
                Some(ExprToken::from(token))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens)
    }
}

fn expr_tokens(node: &JavaSyntaxNode) -> Vec<ExprToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| is_expr_token(token.kind()))
        .map(ExprToken::from)
        .collect()
}

fn is_expr_token(kind: JavaSyntaxKind) -> bool {
    !matches!(kind, JavaSyntaxKind::Semi)
}

#[derive(Debug, Clone)]
struct ExprToken {
    kind: JavaSyntaxKind,
    text: String,
}

impl From<JavaSyntaxToken> for ExprToken {
    fn from(token: JavaSyntaxToken) -> Self {
        Self {
            kind: token.kind(),
            text: token.text().to_string(),
        }
    }
}

fn parse_int_literal(text: &str) -> i64 {
    parse_integer_digits(text.trim_end_matches(['l', 'L']))
}

fn parse_long_literal(text: &str) -> i64 {
    parse_integer_digits(text.trim_end_matches(['l', 'L']))
}

fn parse_integer_digits(text: &str) -> i64 {
    let cleaned = text.replace('_', "");
    if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        i64::from_str_radix(hex, 16).unwrap_or(0)
    } else if let Some(binary) = cleaned
        .strip_prefix("0b")
        .or_else(|| cleaned.strip_prefix("0B"))
    {
        i64::from_str_radix(binary, 2).unwrap_or(0)
    } else {
        cleaned.parse().unwrap_or(0)
    }
}

fn string_literal_value(text: &str) -> String {
    text.strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(text)
        .replace("\\\"", "\"")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\\", "\\")
}

fn is_string_ty(ty: &Ty) -> bool {
    matches!(ty, Ty::Class(name) if name.as_str() == "java/lang/String")
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
