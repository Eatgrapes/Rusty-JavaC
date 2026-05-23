use crate::codegen::CodegenCtx;
use crate::local_var::{load_opcode, store_opcode};
use javac_classfile::Label;
use javac_classfile::MethodWriter;
use javac_hir::hir::*;
use javac_ty::Ty;
use rust_asm::opcodes;

pub fn gen_stmt(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, stmt_id: StmtId) {
    emit_line_number(mw, body, stmt_id);
    let stmt = &body.stmts[stmt_id];
    match stmt {
        Stmt::Return(Some(expr_id)) => {
            crate::expr_gen::gen_expr(mw, ctx, body, *expr_id);
            let expr_ty = crate::expr_gen::expr_ty(ctx, body, *expr_id);
            let return_ty = ctx.return_ty.clone();
            crate::expr_gen::coerce(mw, &expr_ty, &return_ty);
            mw.visit_insn(return_opcode(&return_ty));
        }
        Stmt::Return(None) => {
            mw.visit_insn(opcodes::RETURN);
        }
        Stmt::Expr(expr_id) => {
            crate::expr_gen::gen_expr(mw, ctx, body, *expr_id);
            let ty = crate::expr_gen::expr_ty(ctx, body, *expr_id);
            crate::expr_gen::pop_ty(mw, &ty);
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
                crate::expr_gen::gen_expr(mw, ctx, body, *init);
                let init_ty = crate::expr_gen::expr_ty(ctx, body, *init);
                crate::expr_gen::coerce(mw, &init_ty, &var.ty);
                let store_op = crate::local_var::store_opcode(&var.ty);
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
            crate::expr_gen::gen_expr(mw, ctx, body, *condition);
            mw.visit_jump_insn(opcodes::IFEQ, else_label);
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
            let start_label = Label::new();
            let end_label = Label::new();
            mw.visit_label(start_label);
            crate::expr_gen::gen_expr(mw, ctx, body, *condition);
            mw.visit_jump_insn(opcodes::IFEQ, end_label);
            ctx.continue_labels.push(start_label);
            ctx.break_labels.push(end_label);
            gen_stmt(mw, ctx, body, *loop_body);
            ctx.break_labels.pop();
            ctx.continue_labels.pop();
            if !stmt_definitely_exits(body, *loop_body) {
                mw.visit_jump_insn(opcodes::GOTO, start_label);
            }
            mw.visit_label(end_label);
        }
        Stmt::Do {
            body: loop_body,
            condition,
        } => {
            let start_label = Label::new();
            let continue_label = Label::new();
            let end_label = Label::new();
            mw.visit_label(start_label);
            ctx.continue_labels.push(continue_label);
            ctx.break_labels.push(end_label);
            gen_stmt(mw, ctx, body, *loop_body);
            ctx.break_labels.pop();
            ctx.continue_labels.pop();
            mw.visit_label(continue_label);
            crate::expr_gen::gen_expr(mw, ctx, body, *condition);
            mw.visit_jump_insn(opcodes::IFNE, start_label);
            mw.visit_label(end_label);
        }
        Stmt::For {
            init,
            condition,
            update,
            body: loop_body,
        } => {
            if let Some(init) = init {
                gen_stmt(mw, ctx, body, *init);
            }
            let start_label = Label::new();
            let continue_label = Label::new();
            let end_label = Label::new();
            mw.visit_label(start_label);
            if let Some(condition) = condition {
                crate::expr_gen::gen_expr(mw, ctx, body, *condition);
                mw.visit_jump_insn(opcodes::IFEQ, end_label);
            }
            ctx.continue_labels.push(continue_label);
            ctx.break_labels.push(end_label);
            gen_stmt(mw, ctx, body, *loop_body);
            ctx.break_labels.pop();
            ctx.continue_labels.pop();
            mw.visit_label(continue_label);
            if let Some(update) = update {
                crate::expr_gen::gen_expr(mw, ctx, body, *update);
                let update_ty = crate::expr_gen::expr_ty(ctx, body, *update);
                crate::expr_gen::pop_ty(mw, &update_ty);
            }
            mw.visit_jump_insn(opcodes::GOTO, start_label);
            mw.visit_label(end_label);
        }
        Stmt::ForEach {
            var_type,
            var_name,
            iterable,
            body: loop_body,
        } => {
            emit_array_for_each(mw, ctx, body, var_type, *var_name, *iterable, *loop_body);
        }
        Stmt::Throw(expr_id) => {
            crate::expr_gen::gen_expr(mw, ctx, body, *expr_id);
            mw.visit_insn(opcodes::ATHROW);
        }
        Stmt::Break(_) => {
            if let Some(label) = ctx.break_labels.last() {
                mw.visit_jump_insn(opcodes::GOTO, *label);
            }
        }
        Stmt::Continue(_) => {
            if let Some(label) = ctx.continue_labels.last() {
                mw.visit_jump_insn(opcodes::GOTO, *label);
            }
        }
        Stmt::Switch { selector, cases } => {
            crate::expr_gen::switch::emit_switch_stmt(mw, ctx, body, *selector, cases);
        }
        Stmt::Try(try_stmt) => emit_try_stmt(mw, ctx, body, try_stmt),
        _ => {}
    }
}

fn emit_array_for_each(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    var_type: &Ty,
    var_name: ustr::Ustr,
    iterable: ExprId,
    loop_body: StmtId,
) {
    let array_ty = crate::expr_gen::expr_ty(ctx, body, iterable);
    let element_ty = match array_ty.erasure() {
        Ty::Array(element) => *element,
        _ => var_type.clone(),
    };
    let array_slot = ctx.alloc_temp(&array_ty);
    let index_slot = ctx.alloc_temp(&Ty::Int);
    let var_slot = ctx.alloc_local(var_name, var_type.clone());

    crate::expr_gen::gen_expr(mw, ctx, body, iterable);
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
    mw.visit_insn(crate::expr_gen::array_load_opcode(&element_ty));
    crate::expr_gen::coerce(mw, &element_ty, var_type);
    mw.visit_var_insn(store_opcode(var_type), var_slot);

    ctx.continue_labels.push(continue_label);
    ctx.break_labels.push(end_label);
    gen_stmt(mw, ctx, body, loop_body);
    ctx.break_labels.pop();
    ctx.continue_labels.pop();

    mw.visit_label(continue_label);
    mw.visit_iinc_insn(index_slot, 1);
    mw.visit_jump_insn(opcodes::GOTO, start_label);
    mw.visit_label(end_label);
}

fn emit_try_stmt(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, try_stmt: &TryStmt) {
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
    for stmt in &try_stmt.body.stmts {
        gen_stmt(mw, ctx, body, *stmt);
    }
    mw.visit_label(end_label);
    emit_cleanup(mw, ctx, body, &resources, try_stmt.finally.as_ref());
    mw.visit_jump_insn(opcodes::GOTO, after_label);

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
        for stmt in &catch.body.stmts {
            gen_stmt(mw, ctx, body, *stmt);
        }
        mw.visit_label(catch_end);
        emit_cleanup(mw, ctx, body, &resources, try_stmt.finally.as_ref());
        mw.visit_jump_insn(opcodes::GOTO, after_label);
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

struct ResourceLocal {
    ty: Ty,
    slot: u16,
}

fn emit_try_resources(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    resources: &[TryResource],
) -> Vec<ResourceLocal> {
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
                crate::expr_gen::gen_expr(mw, ctx, body, initializer);
                crate::expr_gen::coerce(
                    mw,
                    &crate::expr_gen::expr_ty(ctx, body, initializer),
                    &resource.ty,
                );
                mw.visit_var_insn(store_opcode(&resource.ty), slot);
            } else {
                crate::expr_gen::push_default_value(mw, &resource.ty);
                mw.visit_var_insn(store_opcode(&resource.ty), slot);
            }
            ResourceLocal {
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
    resources: &[ResourceLocal],
    finally: Option<&Block>,
) {
    emit_resource_closes(mw, resources);
    if let Some(finally) = finally {
        emit_block(mw, ctx, body, finally);
    }
}

fn emit_resource_closes(mw: &mut MethodWriter, resources: &[ResourceLocal]) {
    for resource in resources.iter().rev() {
        let end_label = Label::new();
        mw.visit_var_insn(load_opcode(&resource.ty), resource.slot);
        mw.visit_jump_insn(opcodes::IFNULL, end_label);
        mw.visit_var_insn(load_opcode(&resource.ty), resource.slot);
        mw.visit_method_insn(
            opcodes::INVOKEVIRTUAL,
            &resource.ty.internal_name(),
            "close",
            "()V",
            false,
        );
        mw.visit_label(end_label);
    }
}

fn emit_block(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, block: &Block) {
    for stmt in &block.stmts {
        gen_stmt(mw, ctx, body, *stmt);
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
    crate::expr_gen::gen_expr(mw, ctx, body, binding.source);
    crate::expr_gen::coerce(
        mw,
        &crate::expr_gen::expr_ty(ctx, body, binding.source),
        &binding.ty,
    );
    let slot = ctx.alloc_local(binding.name, binding.ty.clone());
    mw.visit_local_variable(
        binding.name.as_str(),
        &binding.ty.erasure().descriptor(),
        slot,
    );
    mw.visit_var_insn(crate::local_var::store_opcode(&binding.ty), slot);
}

fn return_opcode(ty: &Ty) -> u8 {
    crate::local_var::return_opcode(ty)
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
        Stmt::Switch { cases, .. } => cases.iter().any(|case| match case {
            SwitchCase::Case { body: stmts, .. } | SwitchCase::Default { body: stmts, .. } => stmts
                .last()
                .map(|stmt| stmt_definitely_exits(body, *stmt))
                .unwrap_or(false),
        }),
        _ => false,
    }
}
