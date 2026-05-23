use crate::codegen::CodegenCtx;
use crate::expr_gen::{expr_ty, gen_expr, push_default_value};
use javac_classfile::{Label, MethodWriter};
use javac_hir::hir::{Body, Expr, ExprId, Stmt, SwitchCase};
use rust_asm::opcodes;

pub(super) fn emit_switch_expr(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    selector: ExprId,
    cases: &[SwitchCase],
) {
    let end_label = Label::new();
    let default_label = Label::new();
    let case_labels = build_case_labels(body, cases);
    let switch_ty = switch_result_ty(ctx, body, cases);

    gen_expr(mw, ctx, body, selector);
    let lookup_pairs = case_labels
        .iter()
        .map(|case| (case.key, case.label))
        .collect::<Vec<_>>();
    mw.visit_lookup_switch(default_label, &lookup_pairs);

    for case_label in &case_labels {
        mw.visit_label(case_label.label);
        emit_case_value(mw, ctx, body, &cases[case_label.case_index], &switch_ty);
        mw.visit_jump_insn(opcodes::GOTO, end_label);
    }

    mw.visit_label(default_label);
    if let Some(default_case) = default_case(cases) {
        emit_case_value(mw, ctx, body, default_case, &switch_ty);
    } else {
        push_default_value(mw, &switch_ty);
    }

    mw.visit_label(end_label);
}

struct CaseLabel {
    case_index: usize,
    key: i32,
    label: Label,
}

fn build_case_labels(body: &Body, cases: &[SwitchCase]) -> Vec<CaseLabel> {
    let mut labels = cases
        .iter()
        .enumerate()
        .filter_map(|(case_index, case)| match case {
            SwitchCase::Case { pattern, .. } => int_case_key(body, *pattern).map(|key| CaseLabel {
                case_index,
                key,
                label: Label::new(),
            }),
            SwitchCase::Default { .. } => None,
        })
        .collect::<Vec<_>>();
    labels.sort_by_key(|case| case.key);
    labels
}

fn int_case_key(body: &Body, pattern: ExprId) -> Option<i32> {
    match body.exprs[pattern] {
        Expr::IntLiteral(value) => i32::try_from(value).ok(),
        _ => None,
    }
}

fn default_case(cases: &[SwitchCase]) -> Option<&SwitchCase> {
    cases
        .iter()
        .find(|case| matches!(case, SwitchCase::Default { .. }))
}

fn emit_case_value(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    case: &SwitchCase,
    switch_ty: &javac_ty::Ty,
) {
    if let Some(expr) = case_value(case, body) {
        gen_expr(mw, ctx, body, expr);
        let value_ty = expr_ty(ctx, body, expr);
        crate::expr_gen::coerce(mw, &value_ty, switch_ty);
    } else {
        push_default_value(mw, switch_ty);
    }
}

fn switch_result_ty(ctx: &CodegenCtx, body: &Body, cases: &[SwitchCase]) -> javac_ty::Ty {
    cases
        .iter()
        .find_map(|case| case_value(case, body))
        .map(|expr| expr_ty(ctx, body, expr))
        .unwrap_or_else(|| javac_ty::Ty::Class(ustr::Ustr::from("java/lang/Object")))
}

fn case_value(case: &SwitchCase, body: &Body) -> Option<ExprId> {
    let case_body = match case {
        SwitchCase::Case { body, .. } | SwitchCase::Default { body, .. } => body,
    };

    case_body.iter().find_map(|stmt| match &body.stmts[*stmt] {
        Stmt::Yield(expr) | Stmt::Return(Some(expr)) | Stmt::Expr(expr) => Some(*expr),
        _ => None,
    })
}
