use javac_classfile::MethodWriter;
use crate::codegen::CodegenCtx;
use javac_hir::hir::*;

pub fn gen_method_body(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, block: &Block) {
    for stmt_id in &block.stmts {
        crate::stmt_gen::gen_stmt(mw, ctx, body, *stmt_id);
    }
}