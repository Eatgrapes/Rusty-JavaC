use crate::hir::*;
use crate::lowering::expr::BodyBuilder;
use crate::lowering::syntax::{
    case_pattern_tokens, expr_tokens, first_ident, initializer_tokens, source_line,
    tokens_after_keyword, tokens_in_first_parens,
};
use crate::lowering::types::{is_var_type, lower_type};
use crate::lowering::{LowerError, LowerResult};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode};
use javac_ty::Ty;
use ustr::Ustr;

pub(super) fn lower_block(block: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Block> {
    body.enter_scope();
    let mut stmts = Vec::new();
    for child in block.children() {
        stmts.extend(lower_stmt_nodes(&child, body)?);
    }
    body.exit_scope();
    Ok(Block { stmts })
}

pub(super) fn lower_stmt_as_branch(
    stmt: &JavaSyntaxNode,
    body: &mut BodyBuilder,
) -> LowerResult<StmtId> {
    let stmts = lower_stmt_nodes(stmt, body)?;
    Ok(match stmts.len() {
        0 => body.alloc_stmt_at(Stmt::Empty, source_line(stmt)),
        1 => stmts[0],
        _ => body.alloc_stmt(Stmt::Block(Block { stmts })),
    })
}

fn lower_stmt_nodes(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Vec<StmtId>> {
    let lowered = match stmt.kind() {
        JavaSyntaxKind::LocalVarDecl => lower_local_var_decl(stmt, body)?,
        JavaSyntaxKind::ExprStmt => lower_expr_stmt(stmt, body)?.into_iter().collect(),
        JavaSyntaxKind::ReturnStmt => vec![lower_return_stmt(stmt, body)?],
        JavaSyntaxKind::ThrowStmt => vec![lower_throw_stmt(stmt, body)?],
        JavaSyntaxKind::IfStmt => vec![lower_if_stmt(stmt, body)?],
        JavaSyntaxKind::SwitchStmt => vec![lower_switch_stmt(stmt, body)?],
        JavaSyntaxKind::Block => {
            let block = lower_block(stmt, body)?;
            vec![body.alloc_stmt_at(Stmt::Block(block), source_line(stmt))]
        }
        JavaSyntaxKind::EmptyStmt => vec![body.alloc_stmt_at(Stmt::Empty, source_line(stmt))],
        JavaSyntaxKind::BreakStmt => vec![lower_break_stmt(stmt, body)],
        JavaSyntaxKind::ContinueStmt => vec![lower_continue_stmt(stmt, body)],
        JavaSyntaxKind::YieldStmt => vec![lower_yield_stmt(stmt, body)?],
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
        stmts.push(body.alloc_stmt_at(
            Stmt::LocalVar(LocalVarDecl {
                ty,
                name: Ustr::from(name.text()),
                initializer,
            }),
            source_line(&declarator),
        ));
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
    Ok(Some(
        body.alloc_stmt_at(Stmt::Expr(expr), source_line(stmt)),
    ))
}

fn lower_return_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    if let Some(switch) = stmt
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::SwitchStmt)
    {
        let expr = lower_switch_expr(&switch, body)?;
        return Ok(body.alloc_stmt_at(Stmt::Return(Some(expr)), source_line(stmt)));
    }

    let tokens = tokens_after_keyword(stmt, JavaSyntaxKind::ReturnKw);
    let expr = body.lower_expr_tokens(&tokens)?;
    Ok(body.alloc_stmt_at(Stmt::Return(expr), source_line(stmt)))
}

fn lower_break_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> StmtId {
    body.alloc_stmt_at(Stmt::Break(optional_label(stmt)), source_line(stmt))
}

fn lower_continue_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> StmtId {
    body.alloc_stmt_at(Stmt::Continue(optional_label(stmt)), source_line(stmt))
}

fn lower_yield_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    let tokens = tokens_after_keyword(stmt, JavaSyntaxKind::YieldKw);
    let expr = body
        .lower_expr_tokens(&tokens)?
        .ok_or(LowerError::UnsupportedExpression)?;
    Ok(body.alloc_stmt_at(Stmt::Yield(expr), source_line(stmt)))
}

fn lower_throw_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    let tokens = tokens_after_keyword(stmt, JavaSyntaxKind::ThrowKw);
    let expr = body
        .lower_expr_tokens(&tokens)?
        .ok_or(LowerError::UnsupportedExpression)?;
    Ok(body.alloc_stmt_at(Stmt::Throw(expr), source_line(stmt)))
}

fn lower_if_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    let condition = body
        .lower_expr_tokens(&tokens_in_first_parens(stmt)?)?
        .ok_or(LowerError::UnsupportedExpression)?;
    let pattern_binding = body.pattern_binding(condition);
    let mut branches = stmt.children().filter(is_statement_node);
    let then_node = branches.next().ok_or(LowerError::UnsupportedExpression)?;
    let then_branch = if let Some((name, ty, _)) = pattern_binding {
        body.enter_scope();
        body.define_pattern_local(name, ty);
        let branch = lower_stmt_as_branch(&then_node, body)?;
        body.exit_scope();
        branch
    } else {
        lower_stmt_as_branch(&then_node, body)?
    };
    let else_branch = branches
        .next()
        .map(|node| lower_stmt_as_branch(&node, body))
        .transpose()?;

    Ok(body.alloc_stmt_at(
        Stmt::If {
            condition,
            then_branch,
            else_branch,
        },
        source_line(stmt),
    ))
}

fn lower_switch_expr(switch: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<ExprId> {
    let selector = body
        .lower_expr_tokens(&tokens_in_first_parens(switch)?)?
        .ok_or(LowerError::UnsupportedExpression)?;
    let mut ty = None;
    let cases = lower_switch_cases(switch, body, &mut ty)?;

    let ty = ty.unwrap_or_else(|| Ty::Class(Ustr::from("java/lang/Object")));
    Ok(body.alloc_expr(Expr::Switch {
        selector,
        cases,
        ty,
    }))
}

fn lower_switch_stmt(switch: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    let selector = body
        .lower_expr_tokens(&tokens_in_first_parens(switch)?)?
        .ok_or(LowerError::UnsupportedExpression)?;
    let mut ty = None;
    let cases = lower_switch_cases(switch, body, &mut ty)?;

    Ok(body.alloc_stmt_at(Stmt::Switch { selector, cases }, source_line(switch)))
}

fn lower_switch_cases(
    switch: &JavaSyntaxNode,
    body: &mut BodyBuilder,
    switch_ty: &mut Option<Ty>,
) -> LowerResult<Vec<SwitchCase>> {
    switch
        .children()
        .find(|node| node.kind() == JavaSyntaxKind::SwitchBlock)
        .into_iter()
        .flat_map(|block| block.children())
        .filter(|node| node.kind() == JavaSyntaxKind::SwitchLabel)
        .map(|label| lower_switch_label(&label, body, switch_ty))
        .collect()
}

fn lower_switch_label(
    label: &JavaSyntaxNode,
    body: &mut BodyBuilder,
    switch_ty: &mut Option<Ty>,
) -> LowerResult<SwitchCase> {
    if let Some(rule) = label
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::SwitchRule)
    {
        let rule_body = lower_switch_rule(&rule, body, switch_ty)?;

        if has_token(label, JavaSyntaxKind::DefaultKw) {
            return Ok(SwitchCase::Default {
                body: rule_body,
                is_arrow: true,
            });
        }

        let pattern = body
            .lower_expr_tokens(&case_pattern_tokens(label))?
            .ok_or(LowerError::UnsupportedExpression)?;
        return Ok(SwitchCase::Case {
            pattern,
            body: rule_body,
            is_arrow: true,
        });
    }

    let case_body = lower_switch_colon_body(label, body)?;
    if has_token(label, JavaSyntaxKind::DefaultKw) {
        return Ok(SwitchCase::Default {
            body: case_body,
            is_arrow: false,
        });
    }

    let pattern = body
        .lower_expr_tokens(&case_pattern_tokens(label))?
        .ok_or(LowerError::UnsupportedExpression)?;
    Ok(SwitchCase::Case {
        pattern,
        body: case_body,
        is_arrow: false,
    })
}

fn lower_switch_rule(
    rule: &JavaSyntaxNode,
    body: &mut BodyBuilder,
    switch_ty: &mut Option<Ty>,
) -> LowerResult<Vec<StmtId>> {
    if let Some(block) = rule
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Block)
    {
        let block = lower_block(&block, body)?;
        return Ok(vec![
            body.alloc_stmt_at(Stmt::Block(block), source_line(rule))
        ]);
    }

    let mut stmts = Vec::new();
    for child in rule.children().filter(is_statement_node) {
        stmts.extend(lower_stmt_nodes(&child, body)?);
    }
    if !stmts.is_empty() {
        return Ok(stmts);
    }

    let value = body
        .lower_expr_tokens(&expr_tokens(rule))?
        .ok_or(LowerError::UnsupportedExpression)?;
    switch_ty.get_or_insert_with(|| body.expr_ty(value));
    Ok(vec![
        body.alloc_stmt_at(Stmt::Yield(value), source_line(rule))
    ])
}

fn lower_switch_colon_body(
    label: &JavaSyntaxNode,
    body: &mut BodyBuilder,
) -> LowerResult<Vec<StmtId>> {
    let mut stmts = Vec::new();
    body.enter_scope();
    for child in label.children().filter(is_statement_node) {
        stmts.extend(lower_stmt_nodes(&child, body)?);
    }
    body.exit_scope();
    Ok(stmts)
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
            | JavaSyntaxKind::SwitchStmt
            | JavaSyntaxKind::YieldStmt
    )
}

fn has_token(node: &JavaSyntaxNode, kind: JavaSyntaxKind) -> bool {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == kind)
}

fn optional_label(node: &JavaSyntaxNode) -> Option<Ustr> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == JavaSyntaxKind::Ident)
        .map(|token| Ustr::from(token.text()))
}
