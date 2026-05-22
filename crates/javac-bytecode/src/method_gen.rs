use crate::codegen::CodegenCtx;
use javac_classfile::MethodWriter;
use javac_hir::hir::*;
use javac_ty::Ty;
use rust_asm::opcodes;

pub fn gen_method_body(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, block: &Block) {
    for stmt_id in &block.stmts {
        crate::stmt_gen::gen_stmt(mw, ctx, body, *stmt_id);
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
        _ => false,
    }
}

fn emit_default_return(mw: &mut MethodWriter, ty: &Ty) {
    match ty {
        Ty::Void => mw.visit_insn(opcodes::RETURN),
        Ty::Long => {
            mw.visit_insn(opcodes::LCONST_0);
            mw.visit_insn(opcodes::LRETURN);
        }
        Ty::Float => {
            mw.visit_insn(opcodes::FCONST_0);
            mw.visit_insn(opcodes::FRETURN);
        }
        Ty::Double => {
            mw.visit_insn(opcodes::DCONST_0);
            mw.visit_insn(opcodes::DRETURN);
        }
        Ty::Class(_) | Ty::Array(_) | Ty::TypeVar(_) | Ty::Wildcard(_) | Ty::Intersection(_) => {
            mw.visit_insn(opcodes::ACONST_NULL);
            mw.visit_insn(opcodes::ARETURN);
        }
        _ => {
            mw.visit_insn(opcodes::ICONST_0);
            mw.visit_insn(opcodes::IRETURN);
        }
    }
}
