use crate::codegen::CodegenCtx;
use crate::expr_gen::convert::pop_ty;
use crate::expr_gen::{expr_ty, gen_expr, is_string};
use javac_classfile::{Label, MethodWriter};
use javac_hir::hir::*;
use javac_ty::Ty;
use rust_asm::opcodes;

pub(super) fn emit_binary(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
) {
    let left_ty = expr_ty(ctx, body, left);
    let right_ty = expr_ty(ctx, body, right);

    match op {
        BinaryOp::AndAnd => emit_short_circuit_and(mw, ctx, body, left, right),
        BinaryOp::OrOr => emit_short_circuit_or(mw, ctx, body, left, right),
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
            gen_expr(mw, ctx, body, left);
            gen_expr(mw, ctx, body, right);
            emit_comparison(mw, &op, &left_ty);
        }
        BinaryOp::Add if is_string(&left_ty) || is_string(&right_ty) => {
            emit_string_concat(mw, ctx, body, left, right);
        }
        _ => {
            gen_expr(mw, ctx, body, left);
            gen_expr(mw, ctx, body, right);
            emit_arithmetic(mw, &op, &left_ty);
        }
    }
}

fn emit_string_concat(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    left: ExprId,
    right: ExprId,
) {
    mw.visit_type_insn(opcodes::NEW, "java/lang/StringBuilder");
    mw.visit_insn(opcodes::DUP);
    mw.visit_method_insn(
        opcodes::INVOKESPECIAL,
        "java/lang/StringBuilder",
        "<init>",
        "()V",
        false,
    );
    append_string_part(mw, ctx, body, left);
    append_string_part(mw, ctx, body, right);
    mw.visit_method_insn(
        opcodes::INVOKEVIRTUAL,
        "java/lang/StringBuilder",
        "toString",
        "()Ljava/lang/String;",
        false,
    );
}

fn append_string_part(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, expr_id: ExprId) {
    if let Expr::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = &body.exprs[expr_id]
    {
        let left_ty = expr_ty(ctx, body, *left);
        let right_ty = expr_ty(ctx, body, *right);
        if is_string(&left_ty) || is_string(&right_ty) {
            append_string_part(mw, ctx, body, *left);
            append_string_part(mw, ctx, body, *right);
            return;
        }
    }

    gen_expr(mw, ctx, body, expr_id);
    let ty = expr_ty(ctx, body, expr_id);
    mw.visit_method_insn(
        opcodes::INVOKEVIRTUAL,
        "java/lang/StringBuilder",
        "append",
        &string_builder_append_descriptor(&ty),
        false,
    );
}

fn string_builder_append_descriptor(ty: &Ty) -> String {
    let arg = match ty.erasure() {
        Ty::Boolean => "Z".to_string(),
        Ty::Char => "C".to_string(),
        Ty::Int | Ty::Byte | Ty::Short => "I".to_string(),
        Ty::Long => "J".to_string(),
        Ty::Float => "F".to_string(),
        Ty::Double => "D".to_string(),
        Ty::Class(name) if name.as_str() == "java/lang/String" => "Ljava/lang/String;".to_string(),
        Ty::Class(_) | Ty::Array(_) => "Ljava/lang/Object;".to_string(),
        _ => "Ljava/lang/Object;".to_string(),
    };
    format!("({arg})Ljava/lang/StringBuilder;")
}

pub(super) fn emit_unary(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    op: &UnaryOp,
    operand: ExprId,
) {
    match op {
        UnaryOp::PreInc => super::assign::emit_pre_inc_dec(mw, ctx, body, operand, 1),
        UnaryOp::PreDec => super::assign::emit_pre_inc_dec(mw, ctx, body, operand, -1),
        UnaryOp::Neg => {
            gen_expr(mw, ctx, body, operand);
            mw.visit_insn(neg_opcode(&expr_ty(ctx, body, operand)));
        }
        UnaryOp::Not => {
            gen_expr(mw, ctx, body, operand);
            mw.visit_insn(opcodes::ICONST_1);
            mw.visit_insn(opcodes::IXOR);
        }
        UnaryOp::BitNot => {
            let ty = expr_ty(ctx, body, operand);
            gen_expr(mw, ctx, body, operand);
            if ty == Ty::Long {
                super::literals::emit_long(mw, -1);
                mw.visit_insn(opcodes::LXOR);
            } else {
                mw.visit_insn(opcodes::ICONST_M1);
                mw.visit_insn(opcodes::IXOR);
            }
        }
    }
}

pub(super) fn emit_assign_op(mw: &mut MethodWriter, op: &AssignOp, ty: &Ty) {
    match op {
        AssignOp::Add => mw.visit_insn(add_opcode(ty)),
        AssignOp::Sub => mw.visit_insn(sub_opcode(ty)),
        AssignOp::Mul => mw.visit_insn(mul_opcode(ty)),
        AssignOp::Div => mw.visit_insn(div_opcode(ty)),
        AssignOp::Rem => mw.visit_insn(rem_opcode(ty)),
        AssignOp::And => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LAND
        } else {
            opcodes::IAND
        }),
        AssignOp::Or => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LOR
        } else {
            opcodes::IOR
        }),
        AssignOp::Xor => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LXOR
        } else {
            opcodes::IXOR
        }),
        AssignOp::Shl => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LSHL
        } else {
            opcodes::ISHL
        }),
        AssignOp::Shr => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LSHR
        } else {
            opcodes::ISHR
        }),
        AssignOp::Ushr => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LUSHR
        } else {
            opcodes::IUSHR
        }),
        AssignOp::Plain => {}
    }
}

fn emit_short_circuit_and(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    left: ExprId,
    right: ExprId,
) {
    let false_label = Label::new();
    let end_label = Label::new();
    gen_expr(mw, ctx, body, left);
    mw.visit_jump_insn(opcodes::IFEQ, false_label);
    gen_expr(mw, ctx, body, right);
    mw.visit_jump_insn(opcodes::IFEQ, false_label);
    mw.visit_insn(opcodes::ICONST_1);
    mw.visit_jump_insn(opcodes::GOTO, end_label);
    mw.visit_label(false_label);
    mw.visit_insn(opcodes::ICONST_0);
    mw.visit_label(end_label);
}

fn emit_short_circuit_or(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    left: ExprId,
    right: ExprId,
) {
    let true_label = Label::new();
    let end_label = Label::new();
    gen_expr(mw, ctx, body, left);
    mw.visit_jump_insn(opcodes::IFNE, true_label);
    gen_expr(mw, ctx, body, right);
    mw.visit_jump_insn(opcodes::IFNE, true_label);
    mw.visit_insn(opcodes::ICONST_0);
    mw.visit_jump_insn(opcodes::GOTO, end_label);
    mw.visit_label(true_label);
    mw.visit_insn(opcodes::ICONST_1);
    mw.visit_label(end_label);
}

fn emit_comparison(mw: &mut MethodWriter, op: &BinaryOp, ty: &Ty) {
    let jump = match ty.erasure() {
        Ty::Long => {
            mw.visit_insn(opcodes::LCMP);
            single_value_compare_opcode(op)
        }
        Ty::Float => {
            mw.visit_insn(opcodes::FCMPG);
            single_value_compare_opcode(op)
        }
        Ty::Double => {
            mw.visit_insn(opcodes::DCMPG);
            single_value_compare_opcode(op)
        }
        Ty::Class(_) | Ty::Array(_) => match op {
            BinaryOp::Eq => opcodes::IF_ACMPEQ,
            BinaryOp::Ne => opcodes::IF_ACMPNE,
            _ => opcodes::IF_ACMPEQ,
        },
        _ => int_compare_opcode(op),
    };
    emit_bool_from_jump(mw, jump);
}

fn emit_bool_from_jump(mw: &mut MethodWriter, jump_opcode: u8) {
    let true_label = Label::new();
    let end_label = Label::new();
    mw.visit_jump_insn(jump_opcode, true_label);
    mw.visit_insn(opcodes::ICONST_0);
    mw.visit_jump_insn(opcodes::GOTO, end_label);
    mw.visit_label(true_label);
    mw.visit_insn(opcodes::ICONST_1);
    mw.visit_label(end_label);
}

fn emit_arithmetic(mw: &mut MethodWriter, op: &BinaryOp, ty: &Ty) {
    match op {
        BinaryOp::Add => mw.visit_insn(add_opcode(ty)),
        BinaryOp::Sub => mw.visit_insn(sub_opcode(ty)),
        BinaryOp::Mul => mw.visit_insn(mul_opcode(ty)),
        BinaryOp::Div => mw.visit_insn(div_opcode(ty)),
        BinaryOp::Rem => mw.visit_insn(rem_opcode(ty)),
        BinaryOp::And => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LAND
        } else {
            opcodes::IAND
        }),
        BinaryOp::Or => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LOR
        } else {
            opcodes::IOR
        }),
        BinaryOp::Xor => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LXOR
        } else {
            opcodes::IXOR
        }),
        BinaryOp::Shl => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LSHL
        } else {
            opcodes::ISHL
        }),
        BinaryOp::Shr => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LSHR
        } else {
            opcodes::ISHR
        }),
        BinaryOp::Ushr => mw.visit_insn(if ty == &Ty::Long {
            opcodes::LUSHR
        } else {
            opcodes::IUSHR
        }),
        _ => pop_ty(mw, ty),
    }
}

fn add_opcode(ty: &Ty) -> u8 {
    match ty {
        Ty::Long => opcodes::LADD,
        Ty::Float => opcodes::FADD,
        Ty::Double => opcodes::DADD,
        _ => opcodes::IADD,
    }
}

fn sub_opcode(ty: &Ty) -> u8 {
    match ty {
        Ty::Long => opcodes::LSUB,
        Ty::Float => opcodes::FSUB,
        Ty::Double => opcodes::DSUB,
        _ => opcodes::ISUB,
    }
}

fn mul_opcode(ty: &Ty) -> u8 {
    match ty {
        Ty::Long => opcodes::LMUL,
        Ty::Float => opcodes::FMUL,
        Ty::Double => opcodes::DMUL,
        _ => opcodes::IMUL,
    }
}

fn div_opcode(ty: &Ty) -> u8 {
    match ty {
        Ty::Long => opcodes::LDIV,
        Ty::Float => opcodes::FDIV,
        Ty::Double => opcodes::DDIV,
        _ => opcodes::IDIV,
    }
}

fn rem_opcode(ty: &Ty) -> u8 {
    match ty {
        Ty::Long => opcodes::LREM,
        Ty::Float => opcodes::FREM,
        Ty::Double => opcodes::DREM,
        _ => opcodes::IREM,
    }
}

fn neg_opcode(ty: &Ty) -> u8 {
    match ty {
        Ty::Long => opcodes::LNEG,
        Ty::Float => opcodes::FNEG,
        Ty::Double => opcodes::DNEG,
        _ => opcodes::INEG,
    }
}

fn int_compare_opcode(op: &BinaryOp) -> u8 {
    match op {
        BinaryOp::Eq => opcodes::IF_ICMPEQ,
        BinaryOp::Ne => opcodes::IF_ICMPNE,
        BinaryOp::Lt => opcodes::IF_ICMPLT,
        BinaryOp::Gt => opcodes::IF_ICMPGT,
        BinaryOp::Le => opcodes::IF_ICMPLE,
        BinaryOp::Ge => opcodes::IF_ICMPGE,
        _ => opcodes::IF_ICMPEQ,
    }
}

fn single_value_compare_opcode(op: &BinaryOp) -> u8 {
    match op {
        BinaryOp::Eq => opcodes::IFEQ,
        BinaryOp::Ne => opcodes::IFNE,
        BinaryOp::Lt => opcodes::IFLT,
        BinaryOp::Gt => opcodes::IFGT,
        BinaryOp::Le => opcodes::IFLE,
        BinaryOp::Ge => opcodes::IFGE,
        _ => opcodes::IFEQ,
    }
}
