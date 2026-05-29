use crate::hir::*;
use crate::infer::{self, TypeEnvironment};
use crate::lowering::literal;
use crate::lowering::member::lower_class_members;
use crate::lowering::syntax::ExprToken;
use crate::lowering::types::{TypeResolver, class_type_from_name, lower_type};
use crate::lowering::{LowerError, LowerResult};
use javac_ast::ast::{AstNode, ClassDecl};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode};
use javac_ty::Ty;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use ustr::Ustr;

#[derive(Default)]
pub(super) struct BodyBuilder {
    pub body: Body,
    type_resolver: TypeResolver,
    local_scopes: Vec<HashMap<Ustr, Ty>>,
    pattern_names: HashSet<Ustr>,
    anonymous_types: Vec<Rc<TypeDecl>>,
    anonymous_counter: Rc<Cell<u32>>,
    can_capture_this: bool,
    enclosing_static_owner: Option<Ustr>,
    outer_fields: Vec<CapturedField>,
}

impl BodyBuilder {
    pub(super) fn new(type_resolver: TypeResolver) -> Self {
        let enclosing_static_owner = type_resolver.current_class_name().map(Ustr::from);
        Self {
            enclosing_static_owner,
            type_resolver,
            anonymous_counter: Rc::new(Cell::new(0)),
            ..Self::default()
        }
    }

    pub(super) fn with_anonymous_context(
        type_resolver: TypeResolver,
        anonymous_counter: Rc<Cell<u32>>,
        can_capture_this: bool,
        enclosing_static_owner: Option<Ustr>,
        outer_fields: Vec<CapturedField>,
    ) -> Self {
        Self {
            type_resolver,
            anonymous_counter,
            can_capture_this,
            enclosing_static_owner,
            outer_fields,
            ..Self::default()
        }
    }

    pub(super) fn take_anonymous_types(&mut self) -> Vec<Rc<TypeDecl>> {
        std::mem::take(&mut self.anonymous_types)
    }

    pub(super) fn alloc_stmt(&mut self, stmt: Stmt) -> StmtId {
        self.body.stmts.alloc(stmt)
    }

    pub(super) fn alloc_stmt_at(&mut self, stmt: Stmt, line: u16) -> StmtId {
        let stmt_id = self.alloc_stmt(stmt);
        self.body.stmt_lines.insert(stmt_id, line);
        stmt_id
    }

    pub(super) fn enter_scope(&mut self) {
        self.local_scopes.push(HashMap::new());
    }

    pub(super) fn exit_scope(&mut self) {
        self.local_scopes.pop();
    }

    pub(super) fn define_local(&mut self, name: Ustr, ty: Ty) {
        if self.local_scopes.is_empty() {
            self.enter_scope();
        }
        if let Some(scope) = self.local_scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    pub(super) fn define_pattern_local(&mut self, name: Ustr, ty: Ty) {
        self.pattern_names.insert(name);
        self.define_local(name, ty);
    }

    pub(super) fn local_ty(&self, name: Ustr) -> Option<Ty> {
        self.local_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&name).cloned())
    }

    fn pattern_name_is_out_of_scope(&self, name: Ustr) -> bool {
        self.pattern_names.contains(&name) && self.local_ty(name).is_none()
    }

    pub(super) fn resolve_lambda_target_types(&mut self, method_return_ty: &Ty) {
        let mut targets: Vec<(ExprId, Ty)> = Vec::new();
        for (_, stmt) in self.body.stmts.iter() {
            self.collect_lambda_targets(stmt, method_return_ty, &mut targets);
        }
        for (expr_id, ty) in targets {
            self.apply_lambda_target(expr_id, ty);
        }
    }

    fn apply_lambda_target(&mut self, expr_id: ExprId, ty: Ty) {
        let param_types = self.lambda_param_types(&ty);
        let Expr::Lambda {
            params, target_ty, ..
        } = &mut self.body.exprs[expr_id]
        else {
            return;
        };

        *target_ty = Some(ty);
        if let Some(param_types) = param_types {
            for (param, ty) in params.iter_mut().zip(param_types) {
                param.ty = Some(ty);
            }
        }
    }

    fn lambda_param_types(&self, target_ty: &Ty) -> Option<Vec<Ty>> {
        self.type_resolver
            .functional_interface_method(target_ty)
            .map(|method| method.params)
    }

    fn collect_lambda_targets(
        &self,
        stmt: &Stmt,
        method_return_ty: &Ty,
        targets: &mut Vec<(ExprId, Ty)>,
    ) {
        match stmt {
            Stmt::LocalVar(decl) => {
                if let Some(init) = decl.initializer {
                    self.push_lambda_target(init, &decl.ty, targets);
                }
            }
            Stmt::Return(expr) => {
                if let Some(expr_id) = expr {
                    self.push_lambda_target(*expr_id, method_return_ty, targets);
                }
            }
            Stmt::Expr(expr_id) => {
                self.collect_expr_lambda_targets(*expr_id, targets);
            }
            Stmt::Block(block) => {
                for &s in &block.stmts {
                    self.collect_lambda_targets(&self.body.stmts[s], method_return_ty, targets);
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_lambda_targets(
                    &self.body.stmts[*then_branch],
                    method_return_ty,
                    targets,
                );
                if let Some(eb) = else_branch {
                    self.collect_lambda_targets(&self.body.stmts[*eb], method_return_ty, targets);
                }
            }
            Stmt::For { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::While { body, .. }
            | Stmt::Do { body, .. } => {
                self.collect_lambda_targets(&self.body.stmts[*body], method_return_ty, targets);
            }
            Stmt::Labeled { body, .. } => {
                self.collect_lambda_targets(&self.body.stmts[*body], method_return_ty, targets);
            }
            Stmt::Synchronized(_, block) => {
                for &s in &block.stmts {
                    self.collect_lambda_targets(&self.body.stmts[s], method_return_ty, targets);
                }
            }
            Stmt::Try(try_stmt) => {
                for &s in &try_stmt.body.stmts {
                    self.collect_lambda_targets(&self.body.stmts[s], method_return_ty, targets);
                }
                for catch in &try_stmt.catches {
                    for &s in &catch.body.stmts {
                        self.collect_lambda_targets(&self.body.stmts[s], method_return_ty, targets);
                    }
                }
                if let Some(finally) = &try_stmt.finally {
                    for &s in &finally.stmts {
                        self.collect_lambda_targets(&self.body.stmts[s], method_return_ty, targets);
                    }
                }
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    let stmts = match case {
                        SwitchCase::Case { body, .. } | SwitchCase::Default { body, .. } => body,
                    };
                    for &s in stmts {
                        self.collect_lambda_targets(&self.body.stmts[s], method_return_ty, targets);
                    }
                }
            }
            Stmt::Assert {
                condition: _,
                message,
            } => {
                if let Some(msg) = message {
                    self.collect_expr_lambda_targets(*msg, targets);
                }
            }
            Stmt::Throw(expr_id) | Stmt::Yield(expr_id) => {
                self.collect_expr_lambda_targets(*expr_id, targets);
            }
            Stmt::Empty | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn collect_expr_lambda_targets(&self, expr_id: ExprId, targets: &mut Vec<(ExprId, Ty)>) {
        match &self.body.exprs[expr_id] {
            Expr::Assign { target, value, .. } => {
                let target_ty = self.expr_ty(*target);
                self.push_lambda_target(*value, &target_ty, targets);
            }
            Expr::MethodCall {
                target: _,
                method: _,
                args: _,
            } => {}
            Expr::Parens(inner) => {
                self.collect_expr_lambda_targets(*inner, targets);
            }
            _ => {}
        }
    }

    fn push_lambda_target(&self, expr_id: ExprId, target_ty: &Ty, targets: &mut Vec<(ExprId, Ty)>) {
        let unwrapped = self.unwrap_parens(expr_id);
        if matches!(self.body.exprs[unwrapped], Expr::Lambda { .. }) {
            targets.push((unwrapped, target_ty.clone()));
        }
    }

    fn unwrap_parens(&self, mut expr_id: ExprId) -> ExprId {
        loop {
            match &self.body.exprs[expr_id] {
                Expr::Parens(inner) => expr_id = *inner,
                _ => break expr_id,
            }
        }
    }

    pub(super) fn pattern_binding(&self, expr_id: ExprId) -> Option<(Ustr, Ty, ExprId)> {
        match &self.body.exprs[expr_id] {
            Expr::Instanceof {
                expr,
                ty,
                binding: Some(name),
            } => Some((*name, ty.clone(), *expr)),
            Expr::Parens(inner) => self.pattern_binding(*inner),
            _ => None,
        }
    }

    pub(super) fn lower_expr_tokens(
        &mut self,
        tokens: &[ExprToken],
    ) -> LowerResult<Option<ExprId>> {
        if tokens.is_empty() {
            return Ok(None);
        }

        let mut parser = ExprLowerer {
            tokens,
            pos: 0,
            body: self,
        };
        let expr = parser.parse_expr()?;
        if parser.peek().is_some() {
            return Err(parser.unsupported_expression());
        }
        Ok(Some(expr))
    }

    pub(super) fn expr_ty(&self, expr_id: ExprId) -> Ty {
        infer::expr_ty(self, &self.body, expr_id)
    }

    pub(super) fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        self.body.exprs.alloc(expr)
    }

    pub(super) fn lower_type(&self, node: &javac_ast::JavaSyntaxNode) -> LowerResult<Ty> {
        lower_type(node, &self.type_resolver)
    }

    fn resolve_type_name(&self, name: &str) -> LowerResult<Ty> {
        class_type_from_name(name, 1, &self.type_resolver)
    }
}

impl TypeEnvironment for BodyBuilder {
    fn local_ty(&self, name: Ustr) -> Option<Ty> {
        BodyBuilder::local_ty(self, name)
    }

    fn field_ty(&self, name: Ustr) -> Option<Ty> {
        self.outer_fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.ty.clone())
            .or_else(|| {
                self.enclosing_static_owner.and_then(|owner| {
                    self.type_resolver
                        .resolve_static_field(owner.as_str(), name.as_str())
                        .map(|field| field.ty)
                })
            })
    }

    fn resolve_static_field(&self, owner: &str, name: &str) -> Option<Ty> {
        self.type_resolver
            .resolve_static_field(owner, name)
            .map(|field| field.ty)
    }

    fn resolve_instance_method(&self, receiver: &Ty, name: &str, args: &[Ty]) -> Option<Ty> {
        self.type_resolver
            .resolve_instance_method(receiver, name, args)
            .map(|method| method.return_ty)
    }

    fn this_ty(&self) -> Ty {
        self.type_resolver.current_class_ty()
    }

    fn super_ty(&self) -> Ty {
        self.type_resolver.current_super_ty()
    }
}

struct ExprLowerer<'a, 'b> {
    tokens: &'a [ExprToken],
    pos: usize,
    body: &'b mut BodyBuilder,
}

impl ExprLowerer<'_, '_> {
    fn unsupported_expression(&self) -> LowerError {
        if let Some(token) = self.peek() {
            LowerError::UnsupportedExpressionAt {
                line: token.line,
                range: Some(token.range),
            }
        } else if let Some(token) = self.tokens.last() {
            LowerError::UnsupportedExpressionAt {
                line: token.line,
                range: Some(token.range),
            }
        } else {
            LowerError::UnsupportedExpression
        }
    }

    fn parse_expr(&mut self) -> LowerResult<ExprId> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> LowerResult<ExprId> {
        let target = self.parse_ternary()?;
        let Some(op) = self.peek_assign_op() else {
            return Ok(target);
        };

        self.pos += 1;
        let value = self.parse_assignment()?;
        Ok(self.body.alloc_expr(Expr::Assign { target, op, value }))
    }

    fn parse_ternary(&mut self) -> LowerResult<ExprId> {
        let condition = self.parse_binary(1)?;
        if !self.eat(JavaSyntaxKind::Question) {
            return Ok(condition);
        }

        let then_expr = self.parse_expr()?;
        self.expect(JavaSyntaxKind::Colon)?;
        let else_expr = self.parse_ternary()?;
        Ok(self.body.alloc_expr(Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        }))
    }

    fn parse_binary(&mut self, min_prec: u8) -> LowerResult<ExprId> {
        let mut left = self.parse_unary()?;

        loop {
            if self.peek_kind() == Some(JavaSyntaxKind::InstanceofKw) {
                let prec = 7;
                if prec < min_prec {
                    break;
                }
                self.pos += 1;
                let ty = self.parse_type()?;
                let binding = if self.peek_kind() == Some(JavaSyntaxKind::Ident) {
                    Some(Ustr::from(&self.expect_ident()?))
                } else {
                    None
                };
                left = self.body.alloc_expr(Expr::Instanceof {
                    expr: left,
                    ty,
                    binding,
                });
                continue;
            }

            let Some((op, prec)) = self.peek_binary_op() else {
                break;
            };
            if prec < min_prec {
                break;
            }
            self.pos += 1;

            let right = self.parse_binary(prec + 1)?;
            left = self.body.alloc_expr(Expr::Binary { op, left, right });
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> LowerResult<ExprId> {
        if self.eat(JavaSyntaxKind::Plus) {
            return self.parse_unary();
        }
        if self.looks_like_cast() {
            self.expect(JavaSyntaxKind::LParen)?;
            let ty = self.parse_type()?;
            self.expect(JavaSyntaxKind::RParen)?;
            let expr = self.parse_unary()?;
            return Ok(self.body.alloc_expr(Expr::Cast { ty, expr }));
        }
        if self.eat(JavaSyntaxKind::Minus) {
            let operand = self.parse_unary()?;
            return Ok(self.body.alloc_expr(Expr::Unary {
                op: UnaryOp::Neg,
                operand,
            }));
        }
        if self.eat(JavaSyntaxKind::Bang) {
            let operand = self.parse_unary()?;
            return Ok(self.body.alloc_expr(Expr::Unary {
                op: UnaryOp::Not,
                operand,
            }));
        }
        if self.eat(JavaSyntaxKind::Tilde) {
            let operand = self.parse_unary()?;
            return Ok(self.body.alloc_expr(Expr::Unary {
                op: UnaryOp::BitNot,
                operand,
            }));
        }
        if self.eat(JavaSyntaxKind::Inc) {
            let operand = self.parse_unary()?;
            return Ok(self.body.alloc_expr(Expr::Unary {
                op: UnaryOp::PreInc,
                operand,
            }));
        }
        if self.eat(JavaSyntaxKind::Dec) {
            let operand = self.parse_unary()?;
            return Ok(self.body.alloc_expr(Expr::Unary {
                op: UnaryOp::PreDec,
                operand,
            }));
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> LowerResult<ExprId> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.eat(JavaSyntaxKind::LBrack) {
                let index = self.parse_expr()?;
                self.expect(JavaSyntaxKind::RBrack)?;
                expr = self
                    .body
                    .alloc_expr(Expr::ArrayAccess { array: expr, index });
                continue;
            }

            if self.eat(JavaSyntaxKind::Dot) {
                let name = self.expect_ident()?;
                let name = Ustr::from(&name);
                expr = if self.eat(JavaSyntaxKind::LParen) {
                    let args = self.parse_args_after_open_paren()?;
                    self.body.alloc_expr(Expr::MethodCall {
                        target: Some(expr),
                        method: name,
                        args,
                    })
                } else {
                    self.body.alloc_expr(Expr::FieldAccess {
                        target: expr,
                        field: name,
                    })
                };
                continue;
            }

            if self.eat(JavaSyntaxKind::LParen) {
                let args = self.parse_args_after_open_paren()?;
                expr = self.finish_direct_call(expr, args)?;
                continue;
            }

            if self.eat(JavaSyntaxKind::Inc) {
                return Ok(self.body.alloc_expr(Expr::PostInc(expr)));
            }
            if self.eat(JavaSyntaxKind::Dec) {
                return Ok(self.body.alloc_expr(Expr::PostDec(expr)));
            }

            return Ok(expr);
        }
    }

    fn parse_primary(&mut self) -> LowerResult<ExprId> {
        let Some(token) = self.peek().cloned() else {
            return Err(self.unsupported_expression());
        };

        match token.kind {
            JavaSyntaxKind::IntLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::IntLiteral(literal::parse_int_literal(&token.text))))
            }
            JavaSyntaxKind::LongLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::LongLiteral(literal::parse_long_literal(&token.text))))
            }
            JavaSyntaxKind::FloatLiteral => {
                self.pos += 1;
                if literal::has_float_suffix(&token.text) {
                    Ok(self
                        .body
                        .alloc_expr(Expr::FloatLiteral(literal::parse_float_literal(
                            &token.text,
                        ))))
                } else {
                    Ok(self
                        .body
                        .alloc_expr(Expr::DoubleLiteral(literal::parse_double_literal(
                            &token.text,
                        ))))
                }
            }
            JavaSyntaxKind::DoubleLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::DoubleLiteral(literal::parse_double_literal(
                        &token.text,
                    ))))
            }
            JavaSyntaxKind::CharLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::CharLiteral(literal::parse_char_literal(&token.text))))
            }
            JavaSyntaxKind::StringLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::StringLiteral(literal::string_literal_value(
                        &token.text,
                    ))))
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
            JavaSyntaxKind::SuperKw => {
                self.pos += 1;
                Ok(self.body.alloc_expr(Expr::Super))
            }
            JavaSyntaxKind::NewKw => self.parse_new_expr(),
            JavaSyntaxKind::Ident if self.at_ident_lambda() => {
                self.parse_ident_lambda(token.text.as_str())
            }
            JavaSyntaxKind::Ident => {
                let name = self.expect_ident()?;
                let name = Ustr::from(&name);
                if self.body.pattern_name_is_out_of_scope(name) {
                    return Err(LowerError::PatternVariableOutOfScope(name.to_string()));
                }
                if self.peek_kind() == Some(JavaSyntaxKind::Dot)
                    && self.body.local_ty(name).is_none()
                    && let Some(class_name) = self
                        .body
                        .type_resolver
                        .resolve_class_reference(name.as_str())
                {
                    return Ok(self
                        .body
                        .alloc_expr(Expr::ClassName(Ustr::from(&class_name))));
                }
                Ok(self.body.alloc_expr(Expr::Ident(name)))
            }
            JavaSyntaxKind::LParen if self.is_lambda_paren() => self.parse_parenthesized_lambda(),
            JavaSyntaxKind::LParen => {
                self.pos += 1;
                let inner = self.parse_expr()?;
                self.expect(JavaSyntaxKind::RParen)?;
                Ok(self.body.alloc_expr(Expr::Parens(inner)))
            }
            _ => Err(self.unsupported_expression()),
        }
    }

    fn parse_ident_lambda(&mut self, name: &str) -> LowerResult<ExprId> {
        self.pos += 1;
        self.expect(JavaSyntaxKind::Arrow)?;
        self.finish_lambda(vec![lambda_param(Ustr::from(name))])
    }

    fn parse_parenthesized_lambda(&mut self) -> LowerResult<ExprId> {
        self.expect(JavaSyntaxKind::LParen)?;
        let params = self.parse_lambda_params()?;
        self.expect(JavaSyntaxKind::Arrow)?;
        self.finish_lambda(params)
    }

    fn parse_lambda_params(&mut self) -> LowerResult<Vec<LambdaParam>> {
        let mut params = Vec::new();
        if self.eat(JavaSyntaxKind::RParen) {
            return Ok(params);
        }

        loop {
            let name = Ustr::from(&self.expect_ident()?);
            params.push(lambda_param(name));
            if self.eat(JavaSyntaxKind::Comma) {
                continue;
            }
            self.expect(JavaSyntaxKind::RParen)?;
            return Ok(params);
        }
    }

    fn finish_lambda(&mut self, params: Vec<LambdaParam>) -> LowerResult<ExprId> {
        self.body.enter_scope();
        for param in &params {
            self.body
                .define_local(param.name, param.ty.clone().unwrap_or_else(Ty::object));
        }
        let body = self.parse_lambda_body();
        self.body.exit_scope();
        Ok(self.body.alloc_expr(Expr::Lambda {
            params,
            body: body?,
            target_ty: None,
        }))
    }

    fn parse_lambda_body(&mut self) -> LowerResult<LambdaBody> {
        if self.at_lambda_block() {
            self.skip_block_tokens();
            return Ok(LambdaBody::Block(Block { stmts: vec![] }));
        }
        Ok(LambdaBody::Expr(self.parse_expr()?))
    }

    fn parse_new_expr(&mut self) -> LowerResult<ExprId> {
        self.expect(JavaSyntaxKind::NewKw)?;
        let element_type = self.parse_type_base()?;

        if self.eat(JavaSyntaxKind::LParen) {
            let args = self.parse_args_after_open_paren()?;
            if self.peek_kind() == Some(JavaSyntaxKind::LBrace) || self.at_anonymous_body_member() {
                return self.finish_anonymous_object(element_type, args);
            }
            return Ok(self.body.alloc_expr(Expr::NewObject {
                class: element_type,
                args,
                anonymous: None,
            }));
        }

        let mut dimensions = Vec::new();
        while self.eat(JavaSyntaxKind::LBrack) {
            let size = if self.eat(JavaSyntaxKind::RBrack) {
                None
            } else {
                let size = self.parse_expr()?;
                self.expect(JavaSyntaxKind::RBrack)?;
                Some(size)
            };
            dimensions.push(size);
        }

        let initializer = if self.eat(JavaSyntaxKind::LBrace) {
            Some(self.parse_array_initializer()?)
        } else {
            None
        };

        Ok(self.body.alloc_expr(Expr::NewArray {
            element_type,
            dimensions,
            initializer,
        }))
    }

    fn finish_anonymous_object(&mut self, base_type: Ty, args: Vec<ExprId>) -> LowerResult<ExprId> {
        let body_tokens = self.take_anonymous_body_tokens()?;
        let class_body = parse_anonymous_class_body(&body_tokens)?;
        let class_name = self.next_anonymous_class_name()?;
        let base_internal_name = base_type.internal_name();
        let is_interface = self.body.type_resolver.is_interface(&base_internal_name);
        let super_name = if is_interface {
            "java/lang/Object".to_string()
        } else {
            base_internal_name.clone()
        };
        let arg_types = args
            .iter()
            .map(|arg| self.body.expr_ty(*arg))
            .collect::<Vec<_>>();
        let super_params = if is_interface {
            Vec::new()
        } else {
            self.body
                .type_resolver
                .resolve_constructor(&base_type, &arg_types)
                .map(|method| method.params)
                .unwrap_or_else(|| arg_types.clone())
        };
        let captures_this = self.body.can_capture_this;
        let outer_this = captures_this.then(|| OuterThisInfo {
            field_name: Ustr::from("this$0"),
            ty: self.body.type_resolver.current_class_ty(),
        });
        let constructor_params = anonymous_constructor_params(outer_this.as_ref(), &super_params);
        let constructor = anonymous_constructor(
            constructor_params.clone(),
            SuperConstructorCall {
                owner: if is_interface {
                    Ty::object()
                } else {
                    base_type.clone()
                },
                params: super_params.clone(),
                arg_offset: usize::from(captures_this),
            },
        );
        let resolver = self
            .body
            .type_resolver
            .for_anonymous_class(class_name.as_str(), &super_name);
        let mut members =
            lower_class_members(class_body, &[], &resolver, self.body.enclosing_static_owner)?;
        let mut fields = Vec::new();
        if let Some(outer_this) = &outer_this {
            fields.push(outer_this_field(outer_this));
        }
        fields.append(&mut members.fields);
        let mut methods = vec![constructor];
        methods.append(&mut members.methods);

        let anonymous_type = TypeDecl {
            id: HirId(0),
            name: class_name,
            kind: TypeDeclKind::Class,
            access_flags: 0,
            super_class: (!is_interface).then_some(base_type.clone()),
            interfaces: is_interface
                .then_some(base_type.clone())
                .into_iter()
                .collect(),
            type_params: Vec::new(),
            generic_signature: None,
            fields,
            methods,
            inner_types: members.inner_types,
            anonymous: Some(AnonymousClassInfo {
                super_constructor: SuperConstructorCall {
                    owner: if is_interface {
                        Ty::object()
                    } else {
                        base_type.clone()
                    },
                    params: super_params.clone(),
                    arg_offset: usize::from(captures_this),
                },
                outer_this,
                enclosing_static_owner: self.body.enclosing_static_owner,
                outer_fields: self.body.outer_fields.clone(),
            }),
        };
        self.body.anonymous_types.push(Rc::new(anonymous_type));

        Ok(self.body.alloc_expr(Expr::NewObject {
            class: base_type,
            args,
            anonymous: Some(AnonymousObject {
                class_name,
                constructor_params: super_params,
                captures_enclosing_this: captures_this,
            }),
        }))
    }

    fn next_anonymous_class_name(&mut self) -> LowerResult<Ustr> {
        let owner = self
            .body
            .type_resolver
            .current_class_name()
            .ok_or_else(|| self.unsupported_expression())?;
        let next = self.body.anonymous_counter.get() + 1;
        self.body.anonymous_counter.set(next);
        Ok(Ustr::from(&format!("{owner}${next}")))
    }

    fn take_anonymous_body_tokens(&mut self) -> LowerResult<Vec<ExprToken>> {
        if self.peek_kind() != Some(JavaSyntaxKind::LBrace) {
            return self.take_body_tokens_without_open_brace();
        }

        let mut depth = 0usize;
        let mut tokens = Vec::new();
        loop {
            let Some(token) = self.peek().cloned() else {
                return Err(self.unsupported_expression());
            };
            self.pos += 1;
            match token.kind {
                JavaSyntaxKind::LBrace => depth += 1,
                JavaSyntaxKind::RBrace => {
                    depth = depth.saturating_sub(1);
                    tokens.push(token);
                    if depth == 0 {
                        return Ok(tokens);
                    }
                    continue;
                }
                _ => {}
            }
            tokens.push(token);
        }
    }

    fn take_body_tokens_without_open_brace(&mut self) -> LowerResult<Vec<ExprToken>> {
        let mut depth = 0usize;
        let mut tokens = Vec::new();
        loop {
            let Some(token) = self.peek().cloned() else {
                return Err(self.unsupported_expression());
            };
            self.pos += 1;
            match token.kind {
                JavaSyntaxKind::LBrace => {
                    depth += 1;
                    tokens.push(token);
                }
                JavaSyntaxKind::RBrace if depth == 0 => return Ok(tokens),
                JavaSyntaxKind::RBrace => {
                    depth -= 1;
                    tokens.push(token);
                }
                _ => tokens.push(token),
            }
        }
    }

    fn parse_array_initializer(&mut self) -> LowerResult<ArrayInit> {
        let mut elements = Vec::new();
        if self.eat(JavaSyntaxKind::RBrace) {
            return Ok(ArrayInit { elements });
        }

        loop {
            elements.push(self.parse_expr()?);
            if self.eat(JavaSyntaxKind::Comma) {
                if self.eat(JavaSyntaxKind::RBrace) {
                    break;
                }
                continue;
            }
            self.expect(JavaSyntaxKind::RBrace)?;
            break;
        }

        Ok(ArrayInit { elements })
    }

    fn finish_direct_call(&mut self, expr: ExprId, args: Vec<ExprId>) -> LowerResult<ExprId> {
        match self.body.body.exprs[expr].clone() {
            Expr::Ident(method) => Ok(self.body.alloc_expr(Expr::MethodCall {
                target: None,
                method,
                args,
            })),
            Expr::FieldAccess { target, field } => Ok(self.body.alloc_expr(Expr::MethodCall {
                target: Some(target),
                method: field,
                args,
            })),
            _ => Err(self.unsupported_expression()),
        }
    }

    fn parse_args_after_open_paren(&mut self) -> LowerResult<Vec<ExprId>> {
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

    fn parse_type(&mut self) -> LowerResult<Ty> {
        let mut ty = self.parse_type_base()?;
        while self.eat(JavaSyntaxKind::LBrack) {
            self.expect(JavaSyntaxKind::RBrack)?;
            ty = Ty::Array(Box::new(ty));
        }
        Ok(ty)
    }

    fn parse_type_base(&mut self) -> LowerResult<Ty> {
        let Some(token) = self.peek().cloned() else {
            return Err(self.unsupported_expression());
        };

        let ty = match token.kind {
            JavaSyntaxKind::BooleanKw => Ty::Boolean,
            JavaSyntaxKind::ByteKw => Ty::Byte,
            JavaSyntaxKind::CharKw => Ty::Char,
            JavaSyntaxKind::ShortKw => Ty::Short,
            JavaSyntaxKind::IntKw => Ty::Int,
            JavaSyntaxKind::LongKw => Ty::Long,
            JavaSyntaxKind::FloatKw => Ty::Float,
            JavaSyntaxKind::DoubleKw => Ty::Double,
            JavaSyntaxKind::Ident => return self.parse_type_name(),
            _ => return Err(self.unsupported_expression()),
        };
        self.pos += 1;
        Ok(ty)
    }

    fn looks_like_cast(&self) -> bool {
        if self.peek_kind() != Some(JavaSyntaxKind::LParen) {
            return false;
        }

        let Some(close) = self.matching_rparen(self.pos) else {
            return false;
        };
        if close <= self.pos + 1 {
            return false;
        }

        let first = &self.tokens[self.pos + 1];
        if is_primitive_type_token(first.kind) {
            return true;
        }

        first.kind == JavaSyntaxKind::Ident
            && first.text.chars().next().is_some_and(char::is_uppercase)
            && self.tokens[self.pos + 2..close]
                .iter()
                .all(|token| matches!(token.kind, JavaSyntaxKind::Ident | JavaSyntaxKind::Dot))
    }

    fn matching_rparen(&self, open: usize) -> Option<usize> {
        let mut depth = 0usize;
        for (index, token) in self.tokens.iter().enumerate().skip(open) {
            match token.kind {
                JavaSyntaxKind::LParen => depth += 1,
                JavaSyntaxKind::RParen => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn is_lambda_paren(&self) -> bool {
        let mut depth = 1i32;
        let mut i = self.pos + 1;
        loop {
            if i >= self.tokens.len() {
                return false;
            }
            match self.tokens[i].kind {
                JavaSyntaxKind::LParen => depth += 1,
                JavaSyntaxKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        while i < self.tokens.len()
                            && matches!(
                                self.tokens[i].kind,
                                JavaSyntaxKind::Whitespace | JavaSyntaxKind::Comment
                            )
                        {
                            i += 1;
                        }
                        return i < self.tokens.len()
                            && self.tokens[i].kind == JavaSyntaxKind::Arrow;
                    }
                }
                JavaSyntaxKind::Arrow => return false,
                _ => {}
            }
            i += 1;
        }
    }

    fn peek(&self) -> Option<&ExprToken> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<JavaSyntaxKind> {
        self.peek().map(|token| token.kind)
    }

    fn at_ident_lambda(&self) -> bool {
        self.tokens
            .get(self.pos + 1)
            .is_some_and(|token| token.kind == JavaSyntaxKind::Arrow)
    }

    fn at_lambda_block(&self) -> bool {
        self.peek_kind() == Some(JavaSyntaxKind::LBrace)
    }

    fn at_anonymous_body_member(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(
                JavaSyntaxKind::At
                    | JavaSyntaxKind::PublicKw
                    | JavaSyntaxKind::ProtectedKw
                    | JavaSyntaxKind::PrivateKw
                    | JavaSyntaxKind::StaticKw
                    | JavaSyntaxKind::FinalKw
                    | JavaSyntaxKind::AbstractKw
                    | JavaSyntaxKind::NativeKw
                    | JavaSyntaxKind::SynchronizedKw
                    | JavaSyntaxKind::TransientKw
                    | JavaSyntaxKind::VolatileKw
                    | JavaSyntaxKind::ClassKw
                    | JavaSyntaxKind::InterfaceKw
                    | JavaSyntaxKind::EnumKw
                    | JavaSyntaxKind::RecordKw
                    | JavaSyntaxKind::Ident
                    | JavaSyntaxKind::VoidKw
                    | JavaSyntaxKind::BooleanKw
                    | JavaSyntaxKind::ByteKw
                    | JavaSyntaxKind::CharKw
                    | JavaSyntaxKind::ShortKw
                    | JavaSyntaxKind::IntKw
                    | JavaSyntaxKind::LongKw
                    | JavaSyntaxKind::FloatKw
                    | JavaSyntaxKind::DoubleKw
            )
        )
    }

    fn skip_block_tokens(&mut self) {
        let mut depth = 1;
        self.pos += 1;
        while self.pos < self.tokens.len() && depth > 0 {
            match self.tokens[self.pos].kind {
                JavaSyntaxKind::LBrace => depth += 1,
                JavaSyntaxKind::RBrace => depth -= 1,
                _ => {}
            }
            self.pos += 1;
        }
    }

    fn peek_binary_op(&self) -> Option<(BinaryOp, u8)> {
        let token = self.peek()?;
        let op = match token.kind {
            JavaSyntaxKind::PipePipe => (BinaryOp::OrOr, 1),
            JavaSyntaxKind::AmpAmp => (BinaryOp::AndAnd, 2),
            JavaSyntaxKind::Pipe => (BinaryOp::Or, 3),
            JavaSyntaxKind::Caret => (BinaryOp::Xor, 4),
            JavaSyntaxKind::Amp => (BinaryOp::And, 5),
            JavaSyntaxKind::EqEq => (BinaryOp::Eq, 6),
            JavaSyntaxKind::Neq => (BinaryOp::Ne, 6),
            JavaSyntaxKind::Lt => (BinaryOp::Lt, 7),
            JavaSyntaxKind::Gt => (BinaryOp::Gt, 7),
            JavaSyntaxKind::Le => (BinaryOp::Le, 7),
            JavaSyntaxKind::Ge => (BinaryOp::Ge, 7),
            JavaSyntaxKind::LtLt => (BinaryOp::Shl, 8),
            JavaSyntaxKind::GtGt => (BinaryOp::Shr, 8),
            JavaSyntaxKind::GtGtGt => (BinaryOp::Ushr, 8),
            JavaSyntaxKind::Plus => (BinaryOp::Add, 9),
            JavaSyntaxKind::Minus => (BinaryOp::Sub, 9),
            JavaSyntaxKind::Star => (BinaryOp::Mul, 10),
            JavaSyntaxKind::Slash => (BinaryOp::Div, 10),
            JavaSyntaxKind::Percent => (BinaryOp::Rem, 10),
            _ => return None,
        };
        Some(op)
    }

    fn peek_assign_op(&self) -> Option<AssignOp> {
        let token = self.peek()?;
        let op = match token.kind {
            JavaSyntaxKind::Eq => AssignOp::Plain,
            JavaSyntaxKind::PlusEq => AssignOp::Add,
            JavaSyntaxKind::MinusEq => AssignOp::Sub,
            JavaSyntaxKind::StarEq => AssignOp::Mul,
            JavaSyntaxKind::SlashEq => AssignOp::Div,
            JavaSyntaxKind::PercentEq => AssignOp::Rem,
            JavaSyntaxKind::LtLtEq => AssignOp::Shl,
            JavaSyntaxKind::GtGtEq => AssignOp::Shr,
            JavaSyntaxKind::GtGtGtEq => AssignOp::Ushr,
            JavaSyntaxKind::AmpEq => AssignOp::And,
            JavaSyntaxKind::PipeEq => AssignOp::Or,
            JavaSyntaxKind::CaretEq => AssignOp::Xor,
            _ => return None,
        };
        Some(op)
    }

    fn parse_type_name(&mut self) -> LowerResult<Ty> {
        let mut segments = vec![self.expect_ident()?];
        while self.eat(JavaSyntaxKind::Dot) {
            segments.push(self.expect_ident()?);
        }
        self.body.resolve_type_name(&segments.join("."))
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
            Err(self.unsupported_expression())
        }
    }

    fn expect_ident(&mut self) -> LowerResult<String> {
        let Some(token) = self.peek().cloned() else {
            return Err(self.unsupported_expression());
        };
        if token.kind != JavaSyntaxKind::Ident {
            return Err(self.unsupported_expression());
        }
        self.pos += 1;
        Ok(token.text)
    }
}

fn is_primitive_type_token(kind: JavaSyntaxKind) -> bool {
    matches!(
        kind,
        JavaSyntaxKind::BooleanKw
            | JavaSyntaxKind::ByteKw
            | JavaSyntaxKind::CharKw
            | JavaSyntaxKind::ShortKw
            | JavaSyntaxKind::IntKw
            | JavaSyntaxKind::LongKw
            | JavaSyntaxKind::FloatKw
            | JavaSyntaxKind::DoubleKw
    )
}

fn lambda_param(name: Ustr) -> LambdaParam {
    LambdaParam { name, ty: None }
}

fn parse_anonymous_class_body(tokens: &[ExprToken]) -> LowerResult<javac_ast::ast::ClassBody> {
    let body_source = token_source(tokens);
    let source = if tokens
        .first()
        .is_some_and(|token| token.kind == JavaSyntaxKind::LBrace)
    {
        format!("class Anonymous {body_source}")
    } else {
        format!("class Anonymous {{ {body_source} }}")
    };
    let parse = javac_parser::Parser::parse(&source);
    if !parse.errors.is_empty() {
        let first = tokens.first();
        return Err(first
            .map(|token| LowerError::UnsupportedExpressionAt {
                line: token.line,
                range: Some(token.range),
            })
            .unwrap_or(LowerError::UnsupportedExpression));
    }

    let root = JavaSyntaxNode::new_root(parse.green_node);
    ClassDecl::cast(root.clone())
        .or_else(|| root.children().find_map(ClassDecl::cast))
        .and_then(|class| class.body())
        .ok_or_else(|| {
            tokens
                .first()
                .map(|token| LowerError::UnsupportedExpressionAt {
                    line: token.line,
                    range: Some(token.range),
                })
                .unwrap_or(LowerError::UnsupportedExpression)
        })
}

fn token_source(tokens: &[ExprToken]) -> String {
    let mut source = String::new();
    let mut line = 1u16;
    for token in tokens {
        while line < token.line {
            source.push('\n');
            line += 1;
        }
        if !source.ends_with(['\n', ' ', '{', '(']) {
            source.push(' ');
        }
        source.push_str(&token.text);
    }
    source
}

fn anonymous_constructor_params(
    outer_this: Option<&OuterThisInfo>,
    super_params: &[Ty],
) -> Vec<ParamDecl> {
    let mut params = Vec::new();
    if let Some(outer_this) = outer_this {
        params.push(ParamDecl {
            name: outer_this.field_name,
            ty: outer_this.ty.clone(),
        });
    }
    params.extend(
        super_params
            .iter()
            .enumerate()
            .map(|(index, ty)| ParamDecl {
                name: Ustr::from(&format!("arg{index}")),
                ty: ty.clone(),
            }),
    );
    params
}

fn anonymous_constructor(
    params: Vec<ParamDecl>,
    constructor_call: SuperConstructorCall,
) -> MethodDecl {
    let signature = javac_ty::MethodSig::new(
        Ustr::from("<init>"),
        params.iter().map(|param| param.ty.clone()).collect(),
        Ty::Void,
    );
    MethodDecl {
        id: HirId(0),
        name: Ustr::from("<init>"),
        params,
        signature,
        access_flags: 0,
        source_line: None,
        generic_signature: None,
        throws: Vec::new(),
        body: Body::default(),
        root_block: Some(Block { stmts: Vec::new() }),
        constructor_call: Some(constructor_call),
    }
}

fn outer_this_field(outer_this: &OuterThisInfo) -> FieldDecl {
    FieldDecl {
        id: HirId(0),
        name: outer_this.field_name,
        ty: outer_this.ty.clone(),
        access_flags: javac_classfile::ACC_PRIVATE
            | javac_classfile::ACC_FINAL
            | javac_classfile::ACC_SYNTHETIC,
        generic_signature: None,
        body: Body::default(),
        initializer: None,
    }
}
