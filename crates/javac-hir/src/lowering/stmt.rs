use crate::hir::*;
use crate::lowering::expr::BodyBuilder;
use crate::lowering::syntax::{
    ExprToken, case_pattern_tokens, expr_tokens, first_ident, initializer_tokens, source_line,
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
        JavaSyntaxKind::ForStmt => vec![lower_for_stmt(stmt, body)?],
        JavaSyntaxKind::WhileStmt => vec![lower_while_stmt(stmt, body)?],
        JavaSyntaxKind::DoStmt => vec![lower_do_stmt(stmt, body)?],
        JavaSyntaxKind::SwitchStmt => vec![lower_switch_stmt(stmt, body)?],
        JavaSyntaxKind::TryStmt => vec![lower_try_stmt(stmt, body)?],
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

fn lower_for_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    if let Some(for_each) = stmt
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::ForEach)
    {
        return lower_for_each_stmt(&for_each, body);
    }

    body.enter_scope();
    let init = stmt
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::ForInit)
        .map(|node| lower_for_init(&node, body))
        .transpose()?
        .flatten();
    let header = for_header_segments(stmt)?;
    let condition = header
        .get(1)
        .filter(|tokens| !tokens.is_empty())
        .map(|tokens| body.lower_expr_tokens(tokens))
        .transpose()?
        .flatten();
    let update = header
        .get(2)
        .filter(|tokens| !tokens.is_empty())
        .map(|tokens| body.lower_expr_tokens(tokens))
        .transpose()?
        .flatten();
    let body_node = stmt
        .children()
        .filter(is_statement_node)
        .last()
        .ok_or(LowerError::UnsupportedExpression)?;
    let loop_body = lower_stmt_as_branch(&body_node, body)?;
    body.exit_scope();

    Ok(body.alloc_stmt_at(
        Stmt::For {
            init,
            condition,
            update,
            body: loop_body,
        },
        source_line(stmt),
    ))
}

fn lower_for_init(init: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Option<StmtId>> {
    if init
        .children()
        .any(|child| child.kind() == JavaSyntaxKind::Type)
    {
        let mut lowered = lower_for_init_var_decl(init, body)?;
        return Ok(lowered.pop());
    }

    let Some(expr) = body.lower_expr_tokens(&expr_tokens(init))? else {
        return Ok(None);
    };
    Ok(Some(
        body.alloc_stmt_at(Stmt::Expr(expr), source_line(init)),
    ))
}

fn lower_for_init_var_decl(
    init: &JavaSyntaxNode,
    body: &mut BodyBuilder,
) -> LowerResult<Vec<StmtId>> {
    let declared_ty = init
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Type)
        .ok_or(LowerError::MissingType)?;
    let ty = lower_type(&declared_ty)?;
    let mut stmts = Vec::new();

    for declarator in init
        .descendants()
        .filter(|node| node.kind() == JavaSyntaxKind::VarDeclarator)
    {
        let name = first_ident(&declarator).ok_or(LowerError::MissingMethodName)?;
        let initializer = if let Some(tokens) = initializer_tokens(&declarator) {
            body.lower_expr_tokens(&tokens)?
        } else {
            None
        };
        let name = Ustr::from(name.text());
        body.define_local(name, ty.clone());
        stmts.push(body.alloc_stmt_at(
            Stmt::LocalVar(LocalVarDecl {
                ty: ty.clone(),
                name,
                initializer,
            }),
            source_line(&declarator),
        ));
    }

    Ok(stmts)
}

fn lower_for_each_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    let declared_ty = stmt
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Type)
        .ok_or(LowerError::MissingType)?;
    let var_type = lower_type(&declared_ty)?;
    let var_name = for_each_var_name(stmt)?;
    let iterable = body
        .lower_expr_tokens(&for_each_iterable_tokens(stmt))?
        .ok_or(LowerError::UnsupportedExpression)?;

    body.enter_scope();
    body.define_local(var_name, var_type.clone());
    let body_node = stmt
        .children()
        .filter(is_statement_node)
        .last()
        .ok_or(LowerError::UnsupportedExpression)?;
    let loop_body = lower_stmt_as_branch(&body_node, body)?;
    body.exit_scope();

    Ok(body.alloc_stmt_at(
        Stmt::ForEach {
            var_type,
            var_name,
            iterable,
            body: loop_body,
        },
        source_line(stmt),
    ))
}

fn lower_while_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    let condition = body
        .lower_expr_tokens(&tokens_in_first_parens(stmt)?)?
        .ok_or(LowerError::UnsupportedExpression)?;
    let body_node = stmt
        .children()
        .filter(is_statement_node)
        .last()
        .ok_or(LowerError::UnsupportedExpression)?;
    let loop_body = lower_stmt_as_branch(&body_node, body)?;

    Ok(body.alloc_stmt_at(
        Stmt::While {
            condition,
            body: loop_body,
        },
        source_line(stmt),
    ))
}

fn lower_do_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    let body_node = stmt
        .children()
        .filter(is_statement_node)
        .next()
        .ok_or(LowerError::UnsupportedExpression)?;
    let loop_body = lower_stmt_as_branch(&body_node, body)?;
    let condition = body
        .lower_expr_tokens(&tokens_in_first_parens(stmt)?)?
        .ok_or(LowerError::UnsupportedExpression)?;

    Ok(body.alloc_stmt_at(
        Stmt::Do {
            body: loop_body,
            condition,
        },
        source_line(stmt),
    ))
}

fn lower_try_stmt(stmt: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<StmtId> {
    let resources_node = stmt
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::TryWithResources);

    let try_block = stmt
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Block)
        .ok_or(LowerError::UnsupportedExpression)?;
    let (resources, try_body) = if let Some(resources_node) = resources_node {
        body.enter_scope();
        let resources = lower_try_resources(&resources_node, body)?;
        let try_body = lower_block(&try_block, body)?;
        body.exit_scope();
        (resources, try_body)
    } else {
        (Vec::new(), lower_block(&try_block, body)?)
    };
    let catches = stmt
        .children()
        .filter(|child| child.kind() == JavaSyntaxKind::CatchClause)
        .map(|clause| lower_catch_clause(&clause, body))
        .collect::<LowerResult<Vec<_>>>()?;
    let finally = stmt
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::FinallyClause)
        .map(|clause| lower_finally_clause(&clause, body))
        .transpose()?;

    Ok(body.alloc_stmt_at(
        Stmt::Try(TryStmt {
            resources,
            body: try_body,
            catches,
            finally,
        }),
        source_line(stmt),
    ))
}

fn lower_try_resources(
    resources: &JavaSyntaxNode,
    body: &mut BodyBuilder,
) -> LowerResult<Vec<TryResource>> {
    resources
        .children()
        .filter(|child| child.kind() == JavaSyntaxKind::Resource)
        .map(|resource| lower_try_resource(&resource, body))
        .collect()
}

fn lower_try_resource(
    resource: &JavaSyntaxNode,
    body: &mut BodyBuilder,
) -> LowerResult<TryResource> {
    let declared_ty = resource
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Type)
        .ok_or(LowerError::MissingType)?;
    let explicit_ty = lower_type(&declared_ty)?;
    let initializer = if let Some(tokens) = initializer_tokens(resource) {
        body.lower_expr_tokens(&tokens)?
    } else {
        None
    };
    let ty = local_var_type(is_var_type(&declared_ty), &explicit_ty, initializer, body);
    let name = resource_var_name(resource)?;
    body.define_local(name, ty.clone());

    Ok(TryResource {
        ty,
        name,
        initializer,
    })
}

fn lower_catch_clause(clause: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<CatchClause> {
    let ty_node = clause
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Type)
        .ok_or(LowerError::MissingType)?;
    let exception_type = lower_type(&ty_node)?;
    let var_name = catch_var_name(clause)?;
    let catch_block = clause
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Block)
        .ok_or(LowerError::UnsupportedExpression)?;

    body.enter_scope();
    body.define_local(var_name, exception_type.clone());
    let block = lower_block(&catch_block, body)?;
    body.exit_scope();

    Ok(CatchClause {
        exception_type,
        var_name,
        body: block,
    })
}

fn lower_finally_clause(clause: &JavaSyntaxNode, body: &mut BodyBuilder) -> LowerResult<Block> {
    let block = clause
        .children()
        .find(|child| child.kind() == JavaSyntaxKind::Block)
        .ok_or(LowerError::UnsupportedExpression)?;
    lower_block(&block, body)
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
            body.alloc_stmt_at(Stmt::Block(block), source_line(rule)),
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
        body.alloc_stmt_at(Stmt::Yield(value), source_line(rule)),
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
            | JavaSyntaxKind::ForStmt
            | JavaSyntaxKind::WhileStmt
            | JavaSyntaxKind::DoStmt
            | JavaSyntaxKind::TryStmt
            | JavaSyntaxKind::ReturnStmt
            | JavaSyntaxKind::ThrowStmt
            | JavaSyntaxKind::BreakStmt
            | JavaSyntaxKind::ContinueStmt
            | JavaSyntaxKind::SwitchStmt
            | JavaSyntaxKind::YieldStmt
    )
}

fn for_header_segments(stmt: &JavaSyntaxNode) -> LowerResult<Vec<Vec<ExprToken>>> {
    let mut seen_open = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut segments = vec![Vec::new()];

    for token in stmt
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if !seen_open {
            if token.kind() == JavaSyntaxKind::LParen {
                seen_open = true;
                paren_depth = 1;
            }
            continue;
        }

        match token.kind() {
            JavaSyntaxKind::LParen => {
                paren_depth += 1;
                push_header_token(&mut segments, token);
            }
            JavaSyntaxKind::RParen => {
                paren_depth = paren_depth.saturating_sub(1);
                if paren_depth == 0 {
                    return Ok(segments);
                }
                push_header_token(&mut segments, token);
            }
            JavaSyntaxKind::LBrack => {
                bracket_depth += 1;
                push_header_token(&mut segments, token);
            }
            JavaSyntaxKind::RBrack => {
                bracket_depth = bracket_depth.saturating_sub(1);
                push_header_token(&mut segments, token);
            }
            JavaSyntaxKind::Semi if paren_depth == 1 && bracket_depth == 0 => {
                segments.push(Vec::new());
            }
            _ => push_header_token(&mut segments, token),
        }
    }

    Err(LowerError::UnsupportedExpression)
}

fn push_header_token(segments: &mut [Vec<ExprToken>], token: javac_ast::JavaSyntaxToken) {
    if matches!(
        token.kind(),
        JavaSyntaxKind::Whitespace | JavaSyntaxKind::Comment
    ) {
        return;
    }
    if let Some(segment) = segments.last_mut() {
        segment.push(ExprToken::from(token));
    }
}

fn for_each_var_name(stmt: &JavaSyntaxNode) -> LowerResult<Ustr> {
    stmt.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .take_while(|token| token.kind() != JavaSyntaxKind::Colon)
        .filter(|token| token.kind() == JavaSyntaxKind::Ident)
        .last()
        .map(|token| Ustr::from(token.text()))
        .ok_or(LowerError::MissingMethodName)
}

fn for_each_iterable_tokens(stmt: &JavaSyntaxNode) -> Vec<ExprToken> {
    let mut seen_colon = false;
    let mut paren_depth = 0usize;
    let mut tokens = Vec::new();

    for token in stmt
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if !seen_colon {
            seen_colon = token.kind() == JavaSyntaxKind::Colon;
            continue;
        }

        match token.kind() {
            JavaSyntaxKind::LParen => {
                paren_depth += 1;
                tokens.push(ExprToken::from(token));
            }
            JavaSyntaxKind::RParen if paren_depth == 0 => break,
            JavaSyntaxKind::RParen => {
                paren_depth = paren_depth.saturating_sub(1);
                tokens.push(ExprToken::from(token));
            }
            JavaSyntaxKind::Whitespace | JavaSyntaxKind::Comment => {}
            _ => tokens.push(ExprToken::from(token)),
        }
    }

    tokens
}

fn catch_var_name(clause: &JavaSyntaxNode) -> LowerResult<Ustr> {
    clause
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .take_while(|token| token.kind() != JavaSyntaxKind::RParen)
        .filter(|token| token.kind() == JavaSyntaxKind::Ident)
        .last()
        .map(|token| Ustr::from(token.text()))
        .ok_or(LowerError::MissingMethodName)
}

fn resource_var_name(resource: &JavaSyntaxNode) -> LowerResult<Ustr> {
    resource
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .take_while(|token| token.kind() != JavaSyntaxKind::Eq)
        .filter(|token| token.kind() == JavaSyntaxKind::Ident)
        .last()
        .map(|token| Ustr::from(token.text()))
        .ok_or(LowerError::MissingMethodName)
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
