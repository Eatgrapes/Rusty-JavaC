use crate::bytecode::codegen::CodegenCtx;
use crate::bytecode::expr_gen::push_default_value;
use crate::bytecode::local_var::return_opcode;
use crate::classfile::MethodWriter;
use crate::hir::*;
use crate::ty::Ty;

pub fn gen_method_body(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, block: &Block) {
    for stmt_id in &block.stmts {
        crate::bytecode::stmt_gen::gen_stmt(mw, ctx, body, *stmt_id);
    }
    if !block_definitely_exits(body, block) {
        emit_default_return(mw, &ctx.return_ty);
    }
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
        Stmt::Return(_) | Stmt::Throw(_) => true,
        Stmt::Block(block) => block_definitely_exits(body, block),
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
        _ => false,
    }
}

fn emit_default_return(mw: &mut MethodWriter, ty: &Ty) {
    push_default_value(mw, ty);
    mw.visit_insn(return_opcode(ty));
}
