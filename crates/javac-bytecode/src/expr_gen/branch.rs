use crate::codegen::CodegenCtx;
use crate::expr_gen::{expr_ty, gen_expr};
use javac_classfile::{Label, MethodWriter};
use javac_hir::hir::{BinaryOp, Body, Expr, ExprId, UnaryOp};
use javac_ty::Ty;
use rust_asm::opcodes;

pub(crate) fn emit_jump_if_false(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    expr_id: ExprId,
    target: Label,
) {
    match &body.exprs[expr_id] {
        Expr::BoolLiteral(false) => mw.visit_jump_insn(opcodes::GOTO, target),
        Expr::BoolLiteral(true) => {}
        Expr::Unary {
            op: UnaryOp::Not,
            operand,
        } => emit_jump_if_true(mw, ctx, body, *operand, target),
        Expr::Binary {
            op: BinaryOp::AndAnd,
            left,
            right,
        } => {
            emit_jump_if_false(mw, ctx, body, *left, target);
            emit_jump_if_false(mw, ctx, body, *right, target);
        }
        Expr::Binary {
            op: BinaryOp::OrOr,
            left,
            right,
        } => {
            let true_label = Label::new();
            emit_jump_if_true(mw, ctx, body, *left, true_label);
            emit_jump_if_false(mw, ctx, body, *right, target);
            mw.visit_label(true_label);
        }
        Expr::Binary { op, left, right } if is_comparison(op) => {
            emit_comparison_branch(mw, ctx, body, op, *left, *right, BranchSense::False, target);
        }
        _ => {
            gen_expr(mw, ctx, body, expr_id);
            mw.visit_jump_insn(opcodes::IFEQ, target);
        }
    }
}

pub(crate) fn emit_jump_if_true(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    expr_id: ExprId,
    target: Label,
) {
    match &body.exprs[expr_id] {
        Expr::BoolLiteral(true) => mw.visit_jump_insn(opcodes::GOTO, target),
        Expr::BoolLiteral(false) => {}
        Expr::Unary {
            op: UnaryOp::Not,
            operand,
        } => emit_jump_if_false(mw, ctx, body, *operand, target),
        Expr::Binary {
            op: BinaryOp::AndAnd,
            left,
            right,
        } => {
            let false_label = Label::new();
            emit_jump_if_false(mw, ctx, body, *left, false_label);
            emit_jump_if_true(mw, ctx, body, *right, target);
            mw.visit_label(false_label);
        }
        Expr::Binary {
            op: BinaryOp::OrOr,
            left,
            right,
        } => {
            emit_jump_if_true(mw, ctx, body, *left, target);
            emit_jump_if_true(mw, ctx, body, *right, target);
        }
        Expr::Binary { op, left, right } if is_comparison(op) => {
            emit_comparison_branch(mw, ctx, body, op, *left, *right, BranchSense::True, target);
        }
        _ => {
            gen_expr(mw, ctx, body, expr_id);
            mw.visit_jump_insn(opcodes::IFNE, target);
        }
    }
}

#[derive(Clone, Copy)]
enum BranchSense {
    True,
    False,
}

fn emit_comparison_branch(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    op: &BinaryOp,
    left: ExprId,
    right: ExprId,
    sense: BranchSense,
    target: Label,
) {
    let left_ty = expr_ty(ctx, body, left);
    gen_expr(mw, ctx, body, left);
    gen_expr(mw, ctx, body, right);
    match left_ty.erasure() {
        Ty::Long => {
            mw.visit_insn(opcodes::LCMP);
            mw.visit_jump_insn(single_value_branch(op, sense), target);
        }
        Ty::Float => {
            mw.visit_insn(opcodes::FCMPG);
            mw.visit_jump_insn(single_value_branch(op, sense), target);
        }
        Ty::Double => {
            mw.visit_insn(opcodes::DCMPG);
            mw.visit_jump_insn(single_value_branch(op, sense), target);
        }
        Ty::Class(_) | Ty::Array(_) => {
            mw.visit_jump_insn(reference_branch(op, sense), target);
        }
        _ => {
            mw.visit_jump_insn(int_branch(op, sense), target);
        }
    }
}

fn is_comparison(op: &BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge
    )
}

fn int_branch(op: &BinaryOp, sense: BranchSense) -> u8 {
    match (op, sense) {
        (BinaryOp::Eq, BranchSense::True) | (BinaryOp::Ne, BranchSense::False) => {
            opcodes::IF_ICMPEQ
        }
        (BinaryOp::Ne, BranchSense::True) | (BinaryOp::Eq, BranchSense::False) => {
            opcodes::IF_ICMPNE
        }
        (BinaryOp::Lt, BranchSense::True) | (BinaryOp::Ge, BranchSense::False) => {
            opcodes::IF_ICMPLT
        }
        (BinaryOp::Gt, BranchSense::True) | (BinaryOp::Le, BranchSense::False) => {
            opcodes::IF_ICMPGT
        }
        (BinaryOp::Le, BranchSense::True) | (BinaryOp::Gt, BranchSense::False) => {
            opcodes::IF_ICMPLE
        }
        (BinaryOp::Ge, BranchSense::True) | (BinaryOp::Lt, BranchSense::False) => {
            opcodes::IF_ICMPGE
        }
        _ => opcodes::IF_ICMPNE,
    }
}

fn single_value_branch(op: &BinaryOp, sense: BranchSense) -> u8 {
    match (op, sense) {
        (BinaryOp::Eq, BranchSense::True) | (BinaryOp::Ne, BranchSense::False) => opcodes::IFEQ,
        (BinaryOp::Ne, BranchSense::True) | (BinaryOp::Eq, BranchSense::False) => opcodes::IFNE,
        (BinaryOp::Lt, BranchSense::True) | (BinaryOp::Ge, BranchSense::False) => opcodes::IFLT,
        (BinaryOp::Gt, BranchSense::True) | (BinaryOp::Le, BranchSense::False) => opcodes::IFGT,
        (BinaryOp::Le, BranchSense::True) | (BinaryOp::Gt, BranchSense::False) => opcodes::IFLE,
        (BinaryOp::Ge, BranchSense::True) | (BinaryOp::Lt, BranchSense::False) => opcodes::IFGE,
        _ => opcodes::IFNE,
    }
}

fn reference_branch(op: &BinaryOp, sense: BranchSense) -> u8 {
    match (op, sense) {
        (BinaryOp::Eq, BranchSense::True) | (BinaryOp::Ne, BranchSense::False) => {
            opcodes::IF_ACMPEQ
        }
        (BinaryOp::Ne, BranchSense::True) | (BinaryOp::Eq, BranchSense::False) => {
            opcodes::IF_ACMPNE
        }
        _ => opcodes::IF_ACMPNE,
    }
}
