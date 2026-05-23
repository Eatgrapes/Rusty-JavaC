use crate::hir::*;
use crate::lowering::expr::BodyBuilder;
use crate::lowering::syntax::{
    case_pattern_tokens, expr_tokens, first_ident, initializer_tokens, tokens_after_keyword,
    tokens_in_first_parens,
};
use crate::lowering::types::{is_var_type, lower_type};
use crate::lowering::{LowerError, LowerResult};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode};
use javac_ty::Ty;
use ustr::Ustr;

pub(super) fn lower_block(block: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Block> {
    let mut stmts = Vec::new();
    for child in block.children() {
        stmts.extend(lower_stmt_nodes(&child, body)?);
    }
    Ok(Block { stmts })
}

pub(super) fn lower_stmt_as_branch(
    stmt: &JavaSyntaxNode,
    body: &mut BodyBuilder,
) -> LowerResult<StmtId> {
    let stmts = lower_stmt_nodes(stmt, body)?;
    Ok(match stmts.len() {
        0 => body.alloc_stmt(Stmt::Empty),
        1 => stmts[0],
        _ => body.alloc_stmt(Stmt::Block(Block { stmts })),
    })
}

fn lower_stmt_nodes(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Vec<StmtId>> {
    let lowered = match stmt.kind() {
        JavaSyntaxKind::LocalVarDecl => lower_local_var_decl(stmt, body)?,
        JavaSyntaxKind::ExprStmt => lower_expr_stmt(stmt, body)?.into_iter().collect(),
        JavaSyntaxKind::ReturnStmt => vec![lower_return_stmt(stmt, body)?],
        JavaSyntaxKind::IfStmt => vec![lower_if_stmt(stmt, body)?],
        JavaSyntaxKind::Block => {
            let block = lower_block(stmt, body)?;
            vec![body.alloc_stmt(Stmt::Block(block))]
        }
        JavaSyntaxKind::EmptyStmt => vec![body.alloc_stmt(Stmt::Empty)],
        _ => Vec::new(),
    };
    Ok(lowered)
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

fn lower_return_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    if let Some(switch) = stmt
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::SwitchStmt)
    {
        let expr = lower_switch_expr(&switch, body)?;
        return Ok(body.alloc_stmt(Stmt::Return(Some(expr))));
    }

    let tokens = tokens_after_keyword(stmt, JavaSyntaxKind::ReturnKw);
    let expr = body.lower_expr_tokens(&tokens)?;
    Ok(body.alloc_stmt(Stmt::Return(expr)))
}

fn lower_if_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    let condition = body
        .lower_expr_tokens(&tokens_in_first_parens(stmt)?)?
        .ok_or(LowerError::UnsupportedExpression)?;
    let mut branches = stmt.children().filter(is_statement_node);
    let then_node = branches.next().ok_or(LowerError::UnsupportedExpression)?;
    let then_branch = lower_stmt_as_branch(&then_node, body)?;
    let else_branch = branches
        .next()
        .map(|node| lower_stmt_as_branch(&node, body))
        .transpose()?;

    Ok(body.alloc_stmt(Stmt::If {
        condition,
        then_branch,
        else_branch,
    }))
}

fn lower_switch_expr(switch: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<ExprId> {
    let selector = body
        .lower_expr_tokens(&tokens_in_first_parens(switch)?)?
        .ok_or(LowerError::UnsupportedExpression)?;
    let mut cases = Vec::new();
    let mut ty = None;

    for label in switch
        .descendants()
        .filter(|node| node.kind() == JavaSyntaxKind::SwitchLabel)
    {
        let case = lower_switch_label(&label, body, &mut ty)?;
        cases.push(case);
    }

    let ty = ty.unwrap_or_else(|| Ty::Class(Ustr::from("java/lang/Object")));
    Ok(body.alloc_expr(Expr::Switch {
        selector,
        cases,
        ty,
    }))
}

fn lower_switch_label(
    label: &JavaSyntaxNode,
    body: &mut BodyBuilder,
    switch_ty: &mut Option<Ty>,
) -> LowerResult<SwitchCase> {
    let rule = label
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::SwitchRule)
        .ok_or(LowerError::UnsupportedExpression)?;
    let value = body
        .lower_expr_tokens(&expr_tokens(&rule))?
        .ok_or(LowerError::UnsupportedExpression)?;
    switch_ty.get_or_insert_with(|| body.expr_ty(value));
    let rule_body = vec![body.alloc_stmt(Stmt::Yield(value))];

    if has_token(label, JavaSyntaxKind::DefaultKw) {
        return Ok(SwitchCase::Default {
            body: rule_body,
            is_arrow: true,
        });
    }

    let pattern = body
        .lower_expr_tokens(&case_pattern_tokens(label))?
        .ok_or(LowerError::UnsupportedExpression)?;
    Ok(SwitchCase::Case {
        pattern,
        body: rule_body,
        is_arrow: true,
    })
}

fn is_statement_node(node: &JavaSyntaxNode) -> bool {
    matches!(
        node.kind(),
        JavaSyntaxKind::Block
            | JavaSyntaxKind::ExprStmt
            | JavaSyntaxKind::EmptyStmt
            | JavaSyntaxKind::LocalVarDecl
            | JavaSyntaxKind::IfStmt
            | JavaSyntaxKind::ReturnStmt
            | JavaSyntaxKind::ThrowStmt
            | JavaSyntaxKind::BreakStmt
            | JavaSyntaxKind::ContinueStmt
    )
}

fn has_token(node: &JavaSyntaxNode, kind: JavaSyntaxKind) -> bool {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == kind)
}
