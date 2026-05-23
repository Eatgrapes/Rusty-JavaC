use crate::hir::*;
use crate::lowering::literal;
use crate::lowering::syntax::ExprToken;
use crate::lowering::types::{class_type_from_name, is_string_ty};
use crate::lowering::{LowerError, LowerResult};
use javac_ast::JavaSyntaxKind;
use javac_ty::Ty;
use std::collections::{HashMap, HashSet};
use ustr::Ustr;

#[derive(Default)]
pub(super) struct BodyBuilder {
    pub body: Body,
    local_scopes: Vec<HashMap<Ustr, Ty>>,
    pattern_names: HashSet<Ustr>,
}

impl BodyBuilder {
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
            return Err(LowerError::UnsupportedExpression);
        }
        Ok(Some(expr))
    }

    pub(super) fn expr_ty(&self, expr_id: ExprId) -> Ty {
        match &self.body.exprs[expr_id] {
            Expr::IntLiteral(_) => Ty::Int,
            Expr::LongLiteral(_) => Ty::Long,
            Expr::FloatLiteral(_) => Ty::Float,
            Expr::DoubleLiteral(_) => Ty::Double,
            Expr::BoolLiteral(_) => Ty::Boolean,
            Expr::CharLiteral(_) => Ty::Char,
            Expr::StringLiteral(_) => Ty::Class(Ustr::from("java/lang/String")),
            Expr::NullLiteral => Ty::Class(Ustr::from("java/lang/Object")),
            Expr::This | Expr::Super => Ty::Class(Ustr::from("java/lang/Object")),
            Expr::Ident(name) => self.local_ty(*name).unwrap_or(Ty::Int),
            Expr::NewObject { class, .. } => class.clone(),
            Expr::NewArray { element_type, .. } => Ty::Array(Box::new(element_type.clone())),
            Expr::ArrayAccess { array, .. } => match self.expr_ty(*array) {
                Ty::Array(element) => *element,
                _ => Ty::Int,
            },
            Expr::Binary { op, left, right } => {
                let left_ty = self.expr_ty(*left);
                let right_ty = self.expr_ty(*right);
                match op {
                    BinaryOp::AndAnd
                    | BinaryOp::OrOr
                    | BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Gt
                    | BinaryOp::Le
                    | BinaryOp::Ge => Ty::Boolean,
                    BinaryOp::Add if is_string_ty(&left_ty) || is_string_ty(&right_ty) => {
                        Ty::Class(Ustr::from("java/lang/String"))
                    }
                    _ => numeric_result_ty(&left_ty, &right_ty),
                }
            }
            Expr::Instanceof { .. } => Ty::Boolean,
            Expr::Ternary { then_expr, .. } => self.expr_ty(*then_expr),
            Expr::Switch { ty, .. } => ty.clone(),
            Expr::Cast { ty, .. } => ty.clone(),
            Expr::Assign { target, .. } => self.expr_ty(*target),
            Expr::Parens(inner) => self.expr_ty(*inner),
            _ => Ty::Int,
        }
    }

    pub(super) fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        self.body.exprs.alloc(expr)
    }
}

fn numeric_result_ty(left: &Ty, right: &Ty) -> Ty {
    if left == &Ty::Double || right == &Ty::Double {
        Ty::Double
    } else if left == &Ty::Float || right == &Ty::Float {
        Ty::Float
    } else if left == &Ty::Long || right == &Ty::Long {
        Ty::Long
    } else {
        Ty::Int
    }
}

struct ExprLowerer<'a, 'b> {
    tokens: &'a [ExprToken],
    pos: usize,
    body: &'b mut BodyBuilder,
}

impl ExprLowerer<'_, '_> {
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
            return Err(LowerError::UnsupportedExpression);
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
                Ok(self
                    .body
                    .alloc_expr(Expr::FloatLiteral(literal::parse_float_literal(
                        &token.text,
                    ))))
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
            JavaSyntaxKind::Ident => {
                let name = self.expect_ident()?;
                let name = Ustr::from(&name);
                if self.body.pattern_name_is_out_of_scope(name) {
                    return Err(LowerError::PatternVariableOutOfScope(name.to_string()));
                }
                Ok(self.body.alloc_expr(Expr::Ident(name)))
            }
            JavaSyntaxKind::LParen => {
                self.pos += 1;
                let inner = self.parse_expr()?;
                self.expect(JavaSyntaxKind::RParen)?;
                Ok(self.body.alloc_expr(Expr::Parens(inner)))
            }
            _ => Err(LowerError::UnsupportedExpression),
        }
    }

    fn parse_new_expr(&mut self) -> LowerResult<ExprId> {
        self.expect(JavaSyntaxKind::NewKw)?;
        let element_type = self.parse_type_base()?;

        if self.eat(JavaSyntaxKind::LParen) {
            let args = self.parse_args_after_open_paren()?;
            return Ok(self.body.alloc_expr(Expr::NewObject {
                class: element_type,
                args,
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
            _ => Err(LowerError::UnsupportedExpression),
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
            return Err(LowerError::UnsupportedExpression);
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
            _ => return Err(LowerError::UnsupportedExpression),
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

    fn peek(&self) -> Option<&ExprToken> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<JavaSyntaxKind> {
        self.peek().map(|token| token.kind)
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
        Ok(class_type_from_name(&segments.join(".")))
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
