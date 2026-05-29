use crate::bytecode::codegen::CodegenCtx;
use crate::bytecode::expr_gen::{expr_ty, gen_expr, push_default_value};
use crate::classfile::{Label, MethodWriter};
use crate::hir::{Body, Expr, ExprId, Stmt, StmtId, SwitchCase};
use crate::ty::Ty;
use rust_asm::opcodes;

pub(super) fn emit_switch_expr(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    selector: ExprId,
    cases: &[SwitchCase],
) {
    let result_ty = switch_result_ty(ctx, body, cases);
    emit_switch(
        mw,
        ctx,
        body,
        selector,
        cases,
        SwitchUse::Expression(result_ty),
    );
}

pub(crate) fn emit_switch_stmt(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    selector: ExprId,
    cases: &[SwitchCase],
) {
    emit_switch(mw, ctx, body, selector, cases, SwitchUse::Statement);
}

enum SwitchUse {
    Expression(Ty),
    Statement,
}

fn emit_switch(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    selector: ExprId,
    cases: &[SwitchCase],
    switch_use: SwitchUse,
) {
    let selector_ty = expr_ty(ctx, body, selector);
    if is_string_ty(&selector_ty) {
        emit_text_switch(
            mw,
            ctx,
            body,
            selector,
            cases,
            switch_use,
            TextSwitchKind::String,
        );
    } else if has_enum_case_labels(body, cases) {
        emit_text_switch(
            mw,
            ctx,
            body,
            selector,
            cases,
            switch_use,
            TextSwitchKind::EnumName,
        );
    } else {
        emit_int_switch(mw, ctx, body, selector, cases, switch_use);
    }
}

fn emit_int_switch(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    selector: ExprId,
    cases: &[SwitchCase],
    switch_use: SwitchUse,
) {
    let end_label = Label::new();
    let labels = case_labels(cases);
    let missing_default = missing_default_label(cases, &switch_use);
    let default_target = default_index(cases)
        .and_then(|index| labels[index])
        .or(missing_default)
        .unwrap_or(end_label);
    let mut lookup_pairs = int_lookup_pairs(body, cases, &labels);

    gen_expr(mw, ctx, body, selector);
    lookup_pairs.sort_by_key(|(key, _)| *key);
    mw.visit_lookup_switch(default_target, &lookup_pairs);
    emit_case_bodies(mw, ctx, body, cases, &labels, end_label, &switch_use);
    emit_missing_default(mw, missing_default, &switch_use);
    mw.visit_label(end_label);
}

#[derive(Clone, Copy)]
enum TextSwitchKind {
    String,
    EnumName,
}

fn emit_text_switch(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    selector: ExprId,
    cases: &[SwitchCase],
    switch_use: SwitchUse,
    kind: TextSwitchKind,
) {
    let end_label = Label::new();
    let labels = case_labels(cases);
    let missing_default = missing_default_label(cases, &switch_use);
    let default_target = default_index(cases)
        .and_then(|index| labels[index])
        .or(missing_default)
        .unwrap_or(end_label);
    let selector_ty = expr_ty(ctx, body, selector).erasure();
    let selector_slot = ctx.alloc_temp(&selector_ty);

    gen_expr(mw, ctx, body, selector);
    mw.visit_var_insn(
        crate::bytecode::local_var::store_opcode(&selector_ty),
        selector_slot,
    );
    for (index, case) in cases.iter().enumerate() {
        let Some(label) = labels[index] else {
            continue;
        };
        let Some(key) = text_case_key(body, case, kind) else {
            continue;
        };
        emit_text_case_test(mw, selector_slot, &key, kind, label);
    }
    mw.visit_jump_insn(opcodes::GOTO, default_target);

    emit_case_bodies(mw, ctx, body, cases, &labels, end_label, &switch_use);
    emit_missing_default(mw, missing_default, &switch_use);
    mw.visit_label(end_label);
}

fn emit_text_case_test(
    mw: &mut MethodWriter,
    selector_slot: u16,
    key: &str,
    kind: TextSwitchKind,
    label: Label,
) {
    mw.visit_var_insn(opcodes::ALOAD, selector_slot);
    if matches!(kind, TextSwitchKind::EnumName) {
        mw.visit_method_insn(
            opcodes::INVOKEVIRTUAL,
            "java/lang/Enum",
            "name",
            "()Ljava/lang/String;",
            false,
        );
    }
    mw.visit_ldc_insn_string(key);
    mw.visit_method_insn(
        opcodes::INVOKEVIRTUAL,
        "java/lang/String",
        "equals",
        "(Ljava/lang/Object;)Z",
        false,
    );
    mw.visit_jump_insn(opcodes::IFNE, label);
}

fn emit_case_bodies(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    cases: &[SwitchCase],
    labels: &[Option<Label>],
    end_label: Label,
    switch_use: &SwitchUse,
) {
    let break_target = ctx.control_target(end_label);
    ctx.break_labels.push(break_target);
    for (index, case) in cases.iter().enumerate() {
        if let Some(label) = labels[index] {
            mw.visit_label(label);
        }
        emit_case_body(mw, ctx, body, case, end_label, switch_use);
    }
    ctx.break_labels.pop();
}

fn emit_case_body(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    case: &SwitchCase,
    end_label: Label,
    switch_use: &SwitchUse,
) {
    match switch_use {
        SwitchUse::Expression(result_ty) => {
            emit_case_value(mw, ctx, body, case, result_ty);
            mw.visit_jump_insn(opcodes::GOTO, end_label);
        }
        SwitchUse::Statement => {
            for stmt in case_stmts(case) {
                crate::bytecode::stmt_gen::gen_stmt(mw, ctx, body, *stmt);
            }
            if case_is_arrow(case) && !case_definitely_exits(body, case) {
                mw.visit_jump_insn(opcodes::GOTO, end_label);
            }
        }
    }
}

fn missing_default_label(cases: &[SwitchCase], switch_use: &SwitchUse) -> Option<Label> {
    if matches!(switch_use, SwitchUse::Expression(_)) && default_index(cases).is_none() {
        Some(Label::new())
    } else {
        None
    }
}

fn emit_missing_default(
    mw: &mut MethodWriter,
    missing_default: Option<Label>,
    switch_use: &SwitchUse,
) {
    if let (Some(label), SwitchUse::Expression(result_ty)) = (missing_default, switch_use) {
        mw.visit_label(label);
        push_default_value(mw, result_ty);
    }
}

fn case_labels(cases: &[SwitchCase]) -> Vec<Option<Label>> {
    cases.iter().map(|_| Some(Label::new())).collect()
}

fn int_lookup_pairs(
    body: &Body,
    cases: &[SwitchCase],
    labels: &[Option<Label>],
) -> Vec<(i32, Label)> {
    cases
        .iter()
        .enumerate()
        .filter_map(|(index, case)| match case {
            SwitchCase::Case { pattern, .. } => {
                let label = labels[index]?;
                int_case_key(body, *pattern).map(|key| (key, label))
            }
            SwitchCase::Default { .. } => None,
        })
        .collect()
}

fn int_case_key(body: &Body, pattern: ExprId) -> Option<i32> {
    match body.exprs[pattern] {
        Expr::IntLiteral(value) => i32::try_from(value).ok(),
        Expr::CharLiteral(value) => Some(value as i32),
        _ => None,
    }
}

fn text_case_key(body: &Body, case: &SwitchCase, kind: TextSwitchKind) -> Option<String> {
    let SwitchCase::Case { pattern, .. } = case else {
        return None;
    };
    match kind {
        TextSwitchKind::String => match &body.exprs[*pattern] {
            Expr::StringLiteral(value) => Some(value.to_string()),
            _ => None,
        },
        TextSwitchKind::EnumName => enum_case_key(body, *pattern),
    }
}

fn enum_case_key(body: &Body, pattern: ExprId) -> Option<String> {
    match &body.exprs[pattern] {
        Expr::Ident(name) => Some(name.to_string()),
        Expr::FieldAccess { field, .. } => Some(field.to_string()),
        _ => None,
    }
}

fn has_enum_case_labels(body: &Body, cases: &[SwitchCase]) -> bool {
    cases.iter().any(|case| {
        matches!(
            case,
            SwitchCase::Case { pattern, .. } if enum_case_key(body, *pattern).is_some()
        )
    })
}

fn default_index(cases: &[SwitchCase]) -> Option<usize> {
    cases
        .iter()
        .position(|case| matches!(case, SwitchCase::Default { .. }))
}

fn emit_case_value(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    case: &SwitchCase,
    switch_ty: &Ty,
) {
    if let Some(expr) = case_value(case, body) {
        gen_expr(mw, ctx, body, expr);
        let value_ty = expr_ty(ctx, body, expr);
        crate::bytecode::expr_gen::coerce(mw, &value_ty, switch_ty);
    } else {
        push_default_value(mw, switch_ty);
    }
}

fn switch_result_ty(ctx: &CodegenCtx, body: &Body, cases: &[SwitchCase]) -> Ty {
    cases
        .iter()
        .find_map(|case| case_value(case, body))
        .map(|expr| expr_ty(ctx, body, expr))
        .unwrap_or_else(Ty::object)
}

fn case_value(case: &SwitchCase, body: &Body) -> Option<ExprId> {
    case_stmts(case)
        .iter()
        .find_map(|stmt| match &body.stmts[*stmt] {
            Stmt::Yield(expr) | Stmt::Return(Some(expr)) | Stmt::Expr(expr) => Some(*expr),
            Stmt::Block(block) => block
                .stmts
                .iter()
                .find_map(|stmt| match &body.stmts[*stmt] {
                    Stmt::Yield(expr) | Stmt::Return(Some(expr)) | Stmt::Expr(expr) => Some(*expr),
                    _ => None,
                }),
            _ => None,
        })
}

fn case_stmts(case: &SwitchCase) -> &[StmtId] {
    match case {
        SwitchCase::Case { body, .. } | SwitchCase::Default { body, .. } => body,
    }
}

fn case_is_arrow(case: &SwitchCase) -> bool {
    match case {
        SwitchCase::Case { is_arrow, .. } | SwitchCase::Default { is_arrow, .. } => *is_arrow,
    }
}

fn case_definitely_exits(body: &Body, case: &SwitchCase) -> bool {
    case_stmts(case)
        .last()
        .map(|stmt| {
            matches!(
                body.stmts[*stmt],
                Stmt::Return(_) | Stmt::Throw(_) | Stmt::Break(_)
            )
        })
        .unwrap_or(false)
}

fn is_string_ty(ty: &Ty) -> bool {
    ty.is_string()
}
