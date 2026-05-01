use javac_classfile::MethodWriter;
use rust_asm::opcodes;
use crate::codegen::CodegenCtx;
use javac_hir::hir::*;
use javac_ty::Ty;

pub fn gen_stmt(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, stmt_id: StmtId) {
    let stmt = &body.stmts[stmt_id];
    match stmt {
        Stmt::Return(Some(expr_id)) => {
            crate::expr_gen::gen_expr(mw, ctx, body, *expr_id);
            let ty = body.exprs[*expr_id].ty(&body.exprs);
            mw.visit_insn(return_opcode(&ty));
        }
        Stmt::Return(None) => {
            mw.visit_insn(opcodes::RETURN);
        }
        Stmt::Expr(expr_id) => {
            crate::expr_gen::gen_expr(mw, ctx, body, *expr_id);
            let ty = body.exprs[*expr_id].ty(&body.exprs);
            if !matches!(ty, Ty::Void) {
                if ty.size() == 2 {
                    mw.visit_insn(opcodes::POP2);
                } else {
                    mw.visit_insn(opcodes::POP);
                }
            }
        }
        Stmt::Empty => {}
        Stmt::Block(block) => {
            for s in &block.stmts {
                gen_stmt(mw, ctx, body, *s);
            }
        }
        Stmt::LocalVar(var) => {
            if let Some(init) = &var.initializer {
                crate::expr_gen::gen_expr(mw, ctx, body, *init);
            }
            let slot = ctx.alloc_local(&var.name, var.ty.clone());
            let store_op = crate::local_var::store_opcode(&var.ty);
            mw.visit_var_insn(store_op, slot);
        }
        Stmt::If { condition, then_branch, else_branch } => {
            crate::expr_gen::gen_expr(mw, ctx, body, *condition);
            // TODO: generate proper branch with labels
            gen_stmt(mw, ctx, body, *then_branch);
            if let Some(els) = else_branch {
                gen_stmt(mw, ctx, body, *els);
            }
        }
        Stmt::While { condition, body: loop_body } => {
            crate::expr_gen::gen_expr(mw, ctx, body, *condition);
            gen_stmt(mw, ctx, body, *loop_body);
        }
        Stmt::Throw(expr_id) => {
            crate::expr_gen::gen_expr(mw, ctx, body, *expr_id);
            mw.visit_insn(opcodes::ATHROW);
        }
        _ => {}
    }
}

fn return_opcode(ty: &Ty) -> u8 {
    crate::local_var::return_opcode(ty)
}