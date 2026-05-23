use crate::lowering::{LowerError, LowerResult};
use javac_ast::{JavaSyntaxKind, JavaSyntaxNode, JavaSyntaxToken};

#[derive(Debug, Clone)]
pub(super) struct ExprToken {
    pub kind: JavaSyntaxKind,
    pub text: String,
}

impl From<JavaSyntaxToken> for ExprToken {
    fn from(token: JavaSyntaxToken) -> Self {
        Self {
            kind: token.kind(),
            text: token.text().to_string(),
        }
    }
}

pub(super) fn first_ident(node: &JavaSyntaxNode) -> Option<JavaSyntaxToken> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == JavaSyntaxKind::Ident)
}

pub(super) fn last_ident(node: &JavaSyntaxNode) -> Option<JavaSyntaxToken> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == JavaSyntaxKind::Ident)
        .last()
}

pub(super) fn initializer_tokens(node: &JavaSyntaxNode) -> Option<Vec<ExprToken>> {
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

pub(super) fn expr_tokens(node: &JavaSyntaxNode) -> Vec<ExprToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| is_expr_token(token.kind()))
        .map(ExprToken::from)
        .collect()
}

pub(super) fn tokens_in_first_parens(node: &JavaSyntaxNode) -> LowerResult<Vec<ExprToken>> {
    let mut seen_open = false;
    let mut depth = 0usize;
    let mut tokens = Vec::new();

    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            JavaSyntaxKind::LParen => {
                seen_open = true;
                depth += 1;
                if depth > 1 && is_expr_token(token.kind()) {
                    tokens.push(ExprToken::from(token));
                }
            }
            JavaSyntaxKind::RParen if seen_open => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(tokens);
                }
                if is_expr_token(token.kind()) {
                    tokens.push(ExprToken::from(token));
                }
            }
            _ if seen_open && depth > 0 && is_expr_token(token.kind()) => {
                tokens.push(ExprToken::from(token));
            }
            _ => {}
        }
    }

    Err(LowerError::UnsupportedExpression)
}

pub(super) fn tokens_after_keyword(
    node: &JavaSyntaxNode,
    keyword: JavaSyntaxKind,
) -> Vec<ExprToken> {
    let mut seen_keyword = false;
    let mut tokens = Vec::new();

    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if token.kind() == keyword {
            seen_keyword = true;
            continue;
        }
        if seen_keyword {
            if token.kind() == JavaSyntaxKind::Semi {
                break;
            }
            if is_expr_token(token.kind()) {
                tokens.push(ExprToken::from(token));
            }
        }
    }

    tokens
}

pub(super) fn case_pattern_tokens(label: &JavaSyntaxNode) -> Vec<ExprToken> {
    let mut in_pattern = false;
    let mut tokens = Vec::new();

    for token in label
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            JavaSyntaxKind::CaseKw => in_pattern = true,
            JavaSyntaxKind::Arrow | JavaSyntaxKind::Colon if in_pattern => break,
            _ if in_pattern && is_expr_token(token.kind()) => tokens.push(ExprToken::from(token)),
            _ => {}
        }
    }

    tokens
}

pub(super) fn qualified_name_text(node: &JavaSyntaxNode) -> LowerResult<String> {
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

fn is_expr_token(kind: JavaSyntaxKind) -> bool {
    !matches!(kind, JavaSyntaxKind::Semi)
}
