use crate::hir::*;
use crate::lowering::expr::BodyBuilder;
use crate::lowering::syntax::{expr_tokens, first_ident, initializer_tokens};
use crate::lowering::types::{is_var_type, lower_type};
use crate::lowering::{LowerError, LowerResult};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode};
use javac_ty::Ty;
use ustr::Ustr;

pub(super) fn lower_block(block: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Block> {
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
        let ty = local_var_type(is_var, &explicit_ty, initializer, body);
        body.define_local(Ustr::from(name.text()), ty.clone());
        stmts.push(body.alloc_stmt(Stmt::LocalVar(LocalVarDecl {
            ty,
            name: Ustr::from(name.text()),
            initializer,
        })));
    }

    Ok(stmts)
}

fn local_var_type(
    is_var: bool,
    explicit_ty: &Ty,
    initializer: Option<ExprId>,
    body: &BodyBuilder,
) -> Ty {
    if is_var {
        initializer
            .map(|expr| body.expr_ty(expr))
            .unwrap_or_else(|| Ty::Class(Ustr::from("java/lang/Object")))
    } else {
        explicit_ty.clone()
    }
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
