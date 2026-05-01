use javac_classfile::MethodWriter;
use crate::codegen::CodegenCtx;
use javac_hir::hir::*;

pub fn gen_method_body(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Block) {
    for stmt in &body.stmts {
        crate::stmt_gen::gen_stmt(mw, ctx, stmt);
    }
}