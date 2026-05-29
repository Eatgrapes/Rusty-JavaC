use crate::bytecode::codegen::{CleanupResource, CleanupScope, CodegenCtx};
use crate::bytecode::local_var::{load_opcode, store_opcode};
use crate::classfile::Label;
use crate::classfile::MethodWriter;
use crate::hir::*;
use crate::ty::Ty;
use rust_asm::opcodes;

pub fn gen_stmt(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, stmt_id: StmtId) {
    emit_line_number(mw, body, stmt_id);
    let stmt = &body.stmts[stmt_id];
    match stmt {
        Stmt::Return(Some(expr_id)) => {
            crate::bytecode::expr_gen::gen_expr(mw, ctx, body, *expr_id);
            let expr_ty = crate::bytecode::expr_gen::expr_ty(ctx, body, *expr_id);
            let return_ty = ctx.return_ty.clone();
            crate::bytecode::expr_gen::coerce(mw, &expr_ty, &return_ty);
            let slot = ctx.alloc_temp(&return_ty);
            mw.visit_var_insn(store_opcode(&return_ty), slot);
            emit_abrupt_cleanups(mw, ctx, body);
            mw.visit_var_insn(load_opcode(&return_ty), slot);
            mw.visit_insn(return_opcode(&return_ty));
        }
        Stmt::Return(None) => {
            emit_cleanups_to_depth(mw, ctx, body, 0);
            mw.visit_insn(opcodes::RETURN);
        }
        Stmt::Expr(expr_id) => {
            crate::bytecode::expr_gen::gen_expr_for_effect(mw, ctx, body, *expr_id);
        }
        Stmt::Empty => {}
        Stmt::Block(block) => {
            for s in &block.stmts {
                gen_stmt(mw, ctx, body, *s);
            }
        }
        Stmt::LocalVar(var) => {
            let slot = ctx.alloc_local(var.name, var.ty.clone());
            mw.visit_local_variable(var.name.as_str(), &var.ty.erasure().descriptor(), slot);
            if let Some(init) = &var.initializer {
                crate::bytecode::expr_gen::gen_expr(mw, ctx, body, *init);
                let init_ty = crate::bytecode::expr_gen::expr_ty(ctx, body, *init);
                crate::bytecode::expr_gen::coerce(mw, &init_ty, &var.ty);
                let store_op = crate::bytecode::local_var::store_opcode(&var.ty);
                mw.visit_var_insn(store_op, slot);
            }
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let else_label = Label::new();
            let end_label = Label::new();
            let then_exits = stmt_definitely_exits(body, *then_branch);
            let pattern_binding = pattern_binding(body, *condition);
            crate::bytecode::expr_gen::branch::emit_jump_if_false(
                mw, ctx, body, *condition, else_label,
            );
            if let Some(binding) = pattern_binding {
                emit_pattern_binding(mw, ctx, body, binding);
            }
            gen_stmt(mw, ctx, body, *then_branch);
            if !then_exits {
                mw.visit_jump_insn(opcodes::GOTO, end_label);
            }
            mw.visit_label(else_label);
            if let Some(els) = else_branch {
                gen_stmt(mw, ctx, body, *els);
            }
            if !then_exits {
                mw.visit_label(end_label);
            }
        }
        Stmt::While {
            condition,
            body: loop_body,
        } => {
            emit_while_stmt(mw, ctx, body, *condition, *loop_body, None);
        }
        Stmt::Do {
            body: loop_body,
            condition,
        } => {
            emit_do_stmt(mw, ctx, body, *loop_body, *condition, None);
        }
        Stmt::For {
            init,
            condition,
            update,
            body: loop_body,
        } => {
            emit_for_stmt(mw, ctx, body, *init, *condition, *update, *loop_body, None);
        }
        Stmt::ForEach {
            var_type,
            var_name,
            iterable,
            body: loop_body,
        } => {
            emit_array_for_each(
                mw,
                ctx,
                body,
                ArrayForEach {
                    var_type,
                    var_name: *var_name,
                    iterable: *iterable,
                    loop_body: *loop_body,
                    label: None,
                },
            );
        }
        Stmt::Throw(expr_id) => {
            crate::bytecode::expr_gen::gen_expr(mw, ctx, body, *expr_id);
            mw.visit_insn(opcodes::ATHROW);
        }
        Stmt::Break(label) => {
            if let Some(target) = ctx.find_break_target(*label) {
                emit_cleanups_to_depth(mw, ctx, body, target.cleanup_depth);
                mw.visit_jump_insn(opcodes::GOTO, target.label);
            }
        }
        Stmt::Continue(label) => {
            if let Some(target) = ctx.find_continue_target(*label) {
                emit_cleanups_to_depth(mw, ctx, body, target.cleanup_depth);
                mw.visit_jump_insn(opcodes::GOTO, target.label);
            }
        }
        Stmt::Labeled { label, body: stmt } => emit_labeled_stmt(mw, ctx, body, *label, *stmt),
        Stmt::Switch { selector, cases } => {
            crate::bytecode::expr_gen::switch::emit_switch_stmt(mw, ctx, body, *selector, cases);
        }
        Stmt::Try(try_stmt) => emit_try_stmt(mw, ctx, body, try_stmt),
        _ => {}
    }
}

fn emit_labeled_stmt(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    label: ustr::Ustr,
    stmt_id: StmtId,
) {
    match &body.stmts[stmt_id] {
        Stmt::While {
            condition,
            body: loop_body,
        } => emit_while_stmt(mw, ctx, body, *condition, *loop_body, Some(label)),
        Stmt::Do {
            body: loop_body,
            condition,
        } => emit_do_stmt(mw, ctx, body, *loop_body, *condition, Some(label)),
        Stmt::For {
            init,
            condition,
            update,
            body: loop_body,
        } => emit_for_stmt(
            mw,
            ctx,
            body,
            *init,
            *condition,
            *update,
            *loop_body,
            Some(label),
        ),
        Stmt::ForEach {
            var_type,
            var_name,
            iterable,
            body: loop_body,
        } => emit_array_for_each(
            mw,
            ctx,
            body,
            ArrayForEach {
                var_type,
                var_name: *var_name,
                iterable: *iterable,
                loop_body: *loop_body,
                label: Some(label),
            },
        ),
        _ => {
            let end_label = Label::new();
            let target = ctx.control_target(end_label);
            ctx.labeled_break_labels.push((label, target));
            gen_stmt(mw, ctx, body, stmt_id);
            ctx.labeled_break_labels.pop();
            mw.visit_label(end_label);
        }
    }
}

fn emit_while_stmt(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    condition: ExprId,
    loop_body: StmtId,
    label: Option<ustr::Ustr>,
) {
    if is_true_literal(body, condition) && stmt_is_plain_break(body, loop_body) {
        return;
    }

    let start_label = Label::new();
    let end_label = Label::new();
    mw.visit_label(start_label);
    crate::bytecode::expr_gen::branch::emit_jump_if_false(mw, ctx, body, condition, end_label);
    push_loop_labels(ctx, label, end_label, start_label);
    gen_stmt(mw, ctx, body, loop_body);
    pop_loop_labels(ctx, label);
    if !stmt_definitely_exits(body, loop_body) {
        mw.visit_jump_insn(opcodes::GOTO, start_label);
    }
    mw.visit_label(end_label);
}

fn emit_do_stmt(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    loop_body: StmtId,
    condition: ExprId,
    label: Option<ustr::Ustr>,
) {
    if is_false_literal(body, condition) && stmt_is_plain_continue(body, loop_body) {
        return;
    }

    let start_label = Label::new();
    let continue_label = Label::new();
    let end_label = Label::new();
    mw.visit_label(start_label);
    push_loop_labels(ctx, label, end_label, continue_label);
    gen_stmt(mw, ctx, body, loop_body);
    pop_loop_labels(ctx, label);
    mw.visit_label(continue_label);
    crate::bytecode::expr_gen::branch::emit_jump_if_true(mw, ctx, body, condition, start_label);
    mw.visit_label(end_label);
}

#[allow(clippy::too_many_arguments)]
fn emit_for_stmt(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    init: Option<StmtId>,
    condition: Option<ExprId>,
    update: Option<ExprId>,
    loop_body: StmtId,
    label: Option<ustr::Ustr>,
) {
    if let Some(init) = init {
        gen_stmt(mw, ctx, body, init);
    }
    let start_label = Label::new();
    let continue_label = Label::new();
    let end_label = Label::new();
    mw.visit_label(start_label);
    if let Some(condition) = condition {
        crate::bytecode::expr_gen::branch::emit_jump_if_false(mw, ctx, body, condition, end_label);
    }
    push_loop_labels(ctx, label, end_label, continue_label);
    gen_stmt(mw, ctx, body, loop_body);
    pop_loop_labels(ctx, label);
    mw.visit_label(continue_label);
    if let Some(update) = update {
        crate::bytecode::expr_gen::gen_expr_for_effect(mw, ctx, body, update);
    }
    mw.visit_jump_insn(opcodes::GOTO, start_label);
    mw.visit_label(end_label);
}

fn push_loop_labels(
    ctx: &mut CodegenCtx,
    label: Option<ustr::Ustr>,
    break_label: Label,
    continue_label: Label,
) {
    let break_target = ctx.control_target(break_label);
    let continue_target = ctx.control_target(continue_label);
    ctx.continue_labels.push(continue_target);
    ctx.break_labels.push(break_target);
    if let Some(label) = label {
        ctx.push_labeled_loop(label, break_target, continue_target);
    }
}

fn pop_loop_labels(ctx: &mut CodegenCtx, label: Option<ustr::Ustr>) {
    if label.is_some() {
        ctx.pop_labeled_loop();
    }
    ctx.break_labels.pop();
    ctx.continue_labels.pop();
}

struct ArrayForEach<'a> {
    var_type: &'a Ty,
    var_name: ustr::Ustr,
    iterable: ExprId,
    loop_body: StmtId,
    label: Option<ustr::Ustr>,
}

fn emit_array_for_each(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    for_each: ArrayForEach<'_>,
) {
    let ArrayForEach {
        var_type,
        var_name,
        iterable,
        loop_body,
        label,
    } = for_each;
    let array_ty = crate::bytecode::expr_gen::expr_ty(ctx, body, iterable);
    let element_ty = match array_ty.erasure() {
        Ty::Array(element) => *element,
        _ => var_type.clone(),
    };
    let array_slot = ctx.alloc_temp(&array_ty);
    let index_slot = ctx.alloc_temp(&Ty::Int);
    let var_slot = ctx.alloc_local(var_name, var_type.clone());

    crate::bytecode::expr_gen::gen_expr(mw, ctx, body, iterable);
    mw.visit_var_insn(store_opcode(&array_ty), array_slot);
    mw.visit_insn(opcodes::ICONST_0);
    mw.visit_var_insn(opcodes::ISTORE, index_slot);
    mw.visit_local_variable(
        var_name.as_str(),
        &var_type.erasure().descriptor(),
        var_slot,
    );

    let start_label = Label::new();
    let continue_label = Label::new();
    let end_label = Label::new();
    mw.visit_label(start_label);
    mw.visit_var_insn(opcodes::ILOAD, index_slot);
    mw.visit_var_insn(load_opcode(&array_ty), array_slot);
    mw.visit_insn(opcodes::ARRAYLENGTH);
    mw.visit_jump_insn(opcodes::IF_ICMPGE, end_label);

    mw.visit_var_insn(load_opcode(&array_ty), array_slot);
    mw.visit_var_insn(opcodes::ILOAD, index_slot);
    mw.visit_insn(crate::bytecode::expr_gen::array_load_opcode(&element_ty));
    crate::bytecode::expr_gen::coerce(mw, &element_ty, var_type);
    mw.visit_var_insn(store_opcode(var_type), var_slot);

    push_loop_labels(ctx, label, end_label, continue_label);
    gen_stmt(mw, ctx, body, loop_body);
    pop_loop_labels(ctx, label);

    mw.visit_label(continue_label);
    mw.visit_iinc_insn(index_slot, 1);
    mw.visit_jump_insn(opcodes::GOTO, start_label);
    mw.visit_label(end_label);
}

fn emit_try_stmt(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, try_stmt: &TryStmt) {
    if !try_stmt.resources.is_empty() && try_stmt.catches.is_empty() && try_stmt.finally.is_none() {
        emit_try_with_resources_only(mw, ctx, body, try_stmt);
        return;
    }

    let resources = emit_try_resources(mw, ctx, body, &try_stmt.resources);
    let start_label = Label::new();
    let end_label = Label::new();
    let after_label = Label::new();
    let catch_labels = try_stmt
        .catches
        .iter()
        .map(|_| Label::new())
        .collect::<Vec<_>>();
    let needs_cleanup = try_stmt.finally.is_some() || !resources.is_empty();
    let finally_handler = needs_cleanup.then(Label::new);
    let cleanup_scope = needs_cleanup.then(|| CleanupScope {
        resources: resources.clone(),
        finally: try_stmt.finally.clone(),
    });

    for (catch, label) in try_stmt.catches.iter().zip(&catch_labels) {
        mw.visit_try_catch_block(
            start_label,
            end_label,
            *label,
            Some(&catch.exception_type.internal_name()),
        );
    }
    if let Some(handler) = finally_handler {
        mw.visit_try_catch_block(start_label, end_label, handler, None);
    }

    mw.visit_label(start_label);
    if let Some(scope) = cleanup_scope.clone() {
        ctx.cleanup_scopes.push(scope);
    }
    for stmt in &try_stmt.body.stmts {
        gen_stmt(mw, ctx, body, *stmt);
    }
    if cleanup_scope.is_some() {
        ctx.cleanup_scopes.pop();
    }
    mw.visit_label(end_label);
    if !block_definitely_exits(body, &try_stmt.body) {
        emit_cleanup(mw, ctx, body, &resources, try_stmt.finally.as_ref());
        mw.visit_jump_insn(opcodes::GOTO, after_label);
    }

    for (catch, label) in try_stmt.catches.iter().zip(catch_labels) {
        let catch_end = Label::new();
        if let Some(handler) = finally_handler {
            mw.visit_try_catch_block(label, catch_end, handler, None);
        }

        mw.visit_label(label);
        let slot = ctx.alloc_local(catch.var_name, catch.exception_type.clone());
        mw.visit_local_variable(
            catch.var_name.as_str(),
            &catch.exception_type.erasure().descriptor(),
            slot,
        );
        mw.visit_var_insn(opcodes::ASTORE, slot);
        if let Some(scope) = cleanup_scope.clone() {
            ctx.cleanup_scopes.push(scope);
        }
        for stmt in &catch.body.stmts {
            gen_stmt(mw, ctx, body, *stmt);
        }
        if cleanup_scope.is_some() {
            ctx.cleanup_scopes.pop();
        }
        mw.visit_label(catch_end);
        if !block_definitely_exits(body, &catch.body) {
            emit_cleanup(mw, ctx, body, &resources, try_stmt.finally.as_ref());
            mw.visit_jump_insn(opcodes::GOTO, after_label);
        }
    }

    if let Some(handler) = finally_handler {
        mw.visit_label(handler);
        let throwable = Ty::Class(ustr::Ustr::from("java/lang/Throwable"));
        let slot = ctx.alloc_temp(&throwable);
        mw.visit_var_insn(opcodes::ASTORE, slot);
        emit_cleanup(mw, ctx, body, &resources, try_stmt.finally.as_ref());
        mw.visit_var_insn(opcodes::ALOAD, slot);
        mw.visit_insn(opcodes::ATHROW);
    }

    mw.visit_label(after_label);
}

fn emit_try_with_resources_only(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    try_stmt: &TryStmt,
) {
    let resources = emit_try_resources(mw, ctx, body, &try_stmt.resources);
    let start_label = Label::new();
    let end_label = Label::new();
    let primary_handler = Label::new();
    let rethrow_label = Label::new();
    let after_label = Label::new();

    mw.visit_try_catch_block(
        start_label,
        end_label,
        primary_handler,
        Some("java/lang/Throwable"),
    );

    mw.visit_label(start_label);
    for stmt in &try_stmt.body.stmts {
        gen_stmt(mw, ctx, body, *stmt);
    }
    mw.visit_label(end_label);
    emit_resource_closes_unchecked(mw, &resources);
    mw.visit_jump_insn(opcodes::GOTO, after_label);

    mw.visit_label(primary_handler);
    let throwable = Ty::Class(ustr::Ustr::from("java/lang/Throwable"));
    let primary_slot = ctx.alloc_temp(&throwable);
    mw.visit_var_insn(opcodes::ASTORE, primary_slot);
    emit_resource_closes_suppressed(mw, ctx, &resources, primary_slot);
    mw.visit_label(rethrow_label);
    mw.visit_var_insn(opcodes::ALOAD, primary_slot);
    mw.visit_insn(opcodes::ATHROW);

    mw.visit_label(after_label);
}

fn emit_try_resources(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    resources: &[TryResource],
) -> Vec<CleanupResource> {
    resources
        .iter()
        .map(|resource| {
            let slot = ctx.alloc_local(resource.name, resource.ty.clone());
            mw.visit_local_variable(
                resource.name.as_str(),
                &resource.ty.erasure().descriptor(),
                slot,
            );
            if let Some(initializer) = resource.initializer {
                crate::bytecode::expr_gen::gen_expr(mw, ctx, body, initializer);
                crate::bytecode::expr_gen::coerce(
                    mw,
                    &crate::bytecode::expr_gen::expr_ty(ctx, body, initializer),
                    &resource.ty,
                );
                mw.visit_var_insn(store_opcode(&resource.ty), slot);
            } else {
                crate::bytecode::expr_gen::push_default_value(mw, &resource.ty);
                mw.visit_var_insn(store_opcode(&resource.ty), slot);
            }
            CleanupResource {
                ty: resource.ty.clone(),
                slot,
            }
        })
        .collect()
}

fn emit_cleanup(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    resources: &[CleanupResource],
    finally: Option<&Block>,
) {
    emit_resource_closes(mw, resources);
    if let Some(finally) = finally {
        emit_block(mw, ctx, body, finally);
    }
}

fn emit_resource_closes(mw: &mut MethodWriter, resources: &[CleanupResource]) {
    for resource in resources.iter().rev() {
        emit_resource_close_if_present(mw, resource);
    }
}

fn emit_resource_closes_unchecked(mw: &mut MethodWriter, resources: &[CleanupResource]) {
    for resource in resources.iter().rev() {
        emit_resource_close(mw, resource);
    }
}

fn emit_resource_closes_suppressed(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    resources: &[CleanupResource],
    primary_slot: u16,
) {
    let throwable = Ty::Class(ustr::Ustr::from("java/lang/Throwable"));
    for resource in resources.iter().rev() {
        let close_start = Label::new();
        let close_end = Label::new();
        let close_handler = Label::new();
        let next_resource = Label::new();

        mw.visit_try_catch_block(
            close_start,
            close_end,
            close_handler,
            Some("java/lang/Throwable"),
        );

        mw.visit_label(close_start);
        emit_resource_close(mw, resource);
        mw.visit_label(close_end);
        mw.visit_jump_insn(opcodes::GOTO, next_resource);

        mw.visit_label(close_handler);
        let suppressed_slot = ctx.alloc_temp(&throwable);
        mw.visit_var_insn(opcodes::ASTORE, suppressed_slot);
        mw.visit_var_insn(opcodes::ALOAD, primary_slot);
        mw.visit_var_insn(opcodes::ALOAD, suppressed_slot);
        mw.visit_method_insn(
            opcodes::INVOKEVIRTUAL,
            "java/lang/Throwable",
            "addSuppressed",
            "(Ljava/lang/Throwable;)V",
            false,
        );
        mw.visit_label(next_resource);
    }
}

fn emit_resource_close(mw: &mut MethodWriter, resource: &CleanupResource) {
    mw.visit_var_insn(load_opcode(&resource.ty), resource.slot);
    mw.visit_method_insn(
        opcodes::INVOKEVIRTUAL,
        &resource.ty.internal_name(),
        "close",
        "()V",
        false,
    );
}

fn emit_resource_close_if_present(mw: &mut MethodWriter, resource: &CleanupResource) {
    let end_label = Label::new();
    mw.visit_var_insn(load_opcode(&resource.ty), resource.slot);
    mw.visit_jump_insn(opcodes::IFNULL, end_label);
    emit_resource_close(mw, resource);
    mw.visit_label(end_label);
}

fn emit_block(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, block: &Block) {
    for stmt in &block.stmts {
        gen_stmt(mw, ctx, body, *stmt);
    }
}

fn emit_abrupt_cleanups(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body) {
    emit_cleanups_to_depth(mw, ctx, body, 0);
}

fn emit_cleanups_to_depth(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target_depth: usize,
) {
    let scope_count = ctx.cleanup_scopes.len();
    for index in (target_depth..scope_count).rev() {
        let scope = ctx.cleanup_scopes[index].clone();
        let saved_scopes = ctx.cleanup_scopes.clone();
        ctx.cleanup_scopes.truncate(index);
        emit_cleanup(mw, ctx, body, &scope.resources, scope.finally.as_ref());
        ctx.cleanup_scopes = saved_scopes;
    }
}

fn emit_line_number(mw: &mut MethodWriter, body: &Body, stmt_id: StmtId) {
    if let Some(line) = body.stmt_lines.get(&stmt_id).copied() {
        let label = Label::new();
        mw.visit_label(label);
        mw.visit_line_number(line, label);
    }
}

struct PatternBinding {
    name: ustr::Ustr,
    ty: Ty,
    source: ExprId,
}

fn pattern_binding(body: &Body, expr_id: ExprId) -> Option<PatternBinding> {
    match &body.exprs[expr_id] {
        Expr::Instanceof {
            expr,
            ty,
            binding: Some(name),
        } => Some(PatternBinding {
            name: *name,
            ty: ty.clone(),
            source: *expr,
        }),
        Expr::Parens(inner) => pattern_binding(body, *inner),
        _ => None,
    }
}

fn emit_pattern_binding(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    binding: PatternBinding,
) {
    crate::bytecode::expr_gen::gen_expr(mw, ctx, body, binding.source);
    crate::bytecode::expr_gen::cast(
        mw,
        &crate::bytecode::expr_gen::expr_ty(ctx, body, binding.source),
        &binding.ty,
    );
    let slot = ctx.alloc_local(binding.name, binding.ty.clone());
    mw.visit_local_variable(
        binding.name.as_str(),
        &binding.ty.erasure().descriptor(),
        slot,
    );
    mw.visit_var_insn(crate::bytecode::local_var::store_opcode(&binding.ty), slot);
}

fn return_opcode(ty: &Ty) -> u8 {
    crate::bytecode::local_var::return_opcode(ty)
}

fn block_definitely_exits(body: &Body, block: &Block) -> bool {
    block
        .stmts
        .last()
        .map(|stmt| stmt_definitely_exits(body, *stmt))
        .unwrap_or(false)
}

fn stmt_definitely_exits(body: &Body, stmt_id: StmtId) -> bool {
    match &body.stmts[stmt_id] {
        Stmt::Return(_) | Stmt::Throw(_) | Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::Block(block) => block
            .stmts
            .last()
            .map(|stmt| stmt_definitely_exits(body, *stmt))
            .unwrap_or(false),
        Stmt::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => stmt_definitely_exits(body, *then_branch) && stmt_definitely_exits(body, *else_branch),
        Stmt::Try(try_stmt) => {
            if try_stmt
                .finally
                .as_ref()
                .is_some_and(|finally| block_definitely_exits(body, finally))
            {
                return true;
            }
            block_definitely_exits(body, &try_stmt.body)
                && try_stmt
                    .catches
                    .iter()
                    .all(|catch| block_definitely_exits(body, &catch.body))
        }
        Stmt::Labeled { body: stmt, .. } => stmt_definitely_exits(body, *stmt),
        Stmt::Switch { cases, .. } => cases.iter().any(|case| match case {
            SwitchCase::Case { body: stmts, .. } | SwitchCase::Default { body: stmts, .. } => stmts
                .last()
                .map(|stmt| stmt_definitely_exits(body, *stmt))
                .unwrap_or(false),
        }),
        _ => false,
    }
}

fn is_true_literal(body: &Body, expr_id: ExprId) -> bool {
    matches!(body.exprs[expr_id], Expr::BoolLiteral(true))
}

fn is_false_literal(body: &Body, expr_id: ExprId) -> bool {
    matches!(body.exprs[expr_id], Expr::BoolLiteral(false))
}

fn stmt_is_plain_break(body: &Body, stmt_id: StmtId) -> bool {
    match &body.stmts[stmt_id] {
        Stmt::Break(None) => true,
        Stmt::Block(block) => block.stmts.len() == 1 && stmt_is_plain_break(body, block.stmts[0]),
        _ => false,
    }
}

fn stmt_is_plain_continue(body: &Body, stmt_id: StmtId) -> bool {
    match &body.stmts[stmt_id] {
        Stmt::Continue(None) => true,
        Stmt::Block(block) => {
            block.stmts.len() == 1 && stmt_is_plain_continue(body, block.stmts[0])
        }
        _ => false,
    }
}
