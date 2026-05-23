use crate::hir::*;
use crate::lowering::syntax::ExprToken;
use crate::lowering::types::{class_type_from_name, is_string_ty};
use crate::lowering::{LowerError, LowerResult};
use javac_ast::JavaSyntaxKind;
use javac_ty::Ty;
use std::collections::HashMap;
use ustr::Ustr;

#[derive(Default)]
pub(super) struct BodyBuilder {
    pub body: Body,
    local_types: HashMap<Ustr, Ty>,
}

impl BodyBuilder {
    pub(super) fn alloc_stmt(&mut self, stmt: Stmt) -> StmtId {
        self.body.stmts.alloc(stmt)
    }

    pub(super) fn define_local(&mut self, name: Ustr, ty: Ty) {
        self.local_types.insert(name, ty);
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
            Expr::Ident(name) => self.local_types.get(name).cloned().unwrap_or(Ty::Int),
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
                    _ => left_ty,
                }
            }
            Expr::Instanceof { .. } => Ty::Boolean,
            Expr::Switch { ty, .. } => ty.clone(),
            Expr::Parens(inner) => self.expr_ty(*inner),
            _ => Ty::Int,
        }
    }

    pub(super) fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        self.body.exprs.alloc(expr)
    }
}

struct ExprLowerer<'a, 'b> {
    tokens: &'a [ExprToken],
    pos: usize,
    body: &'b mut BodyBuilder,
}

impl ExprLowerer<'_, '_> {
    fn parse_expr(&mut self) -> LowerResult<ExprId> {
        self.parse_binary(1)
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
                let ty = self.parse_type_name()?;
                left = self.body.alloc_expr(Expr::Instanceof { expr: left, ty });
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
        let expr = self.parse_primary()?;
        if self.eat(JavaSyntaxKind::Inc) {
            return Ok(self.body.alloc_expr(Expr::PostInc(expr)));
        }
        if self.eat(JavaSyntaxKind::Dec) {
            return Ok(self.body.alloc_expr(Expr::PostDec(expr)));
        }
        Ok(expr)
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
            JavaSyntaxKind::FloatLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::FloatLiteral(parse_float_literal(&token.text))))
            }
            JavaSyntaxKind::DoubleLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::DoubleLiteral(parse_double_literal(&token.text))))
            }
            JavaSyntaxKind::CharLiteral => {
                self.pos += 1;
                Ok(self
                    .body
                    .alloc_expr(Expr::CharLiteral(parse_char_literal(&token.text))))
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

fn parse_int_literal(text: &str) -> i64 {
    parse_integer_digits(text.trim_end_matches(['l', 'L']))
}

fn parse_long_literal(text: &str) -> i64 {
    parse_integer_digits(text.trim_end_matches(['l', 'L']))
}

fn parse_float_literal(text: &str) -> f32 {
    text.trim_end_matches(['f', 'F'])
        .replace('_', "")
        .parse()
        .unwrap_or(0.0)
}

fn parse_double_literal(text: &str) -> f64 {
    text.trim_end_matches(['d', 'D'])
        .replace('_', "")
        .parse()
        .unwrap_or(0.0)
}

fn parse_char_literal(text: &str) -> char {
    let value = text
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .unwrap_or(text);
    match value {
        "\\n" => '\n',
        "\\t" => '\t',
        "\\r" => '\r',
        "\\'" => '\'',
        "\\\\" => '\\',
        _ => value.chars().next().unwrap_or('\0'),
    }
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
