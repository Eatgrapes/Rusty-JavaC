use javac_classfile::MethodWriter;
use rust_asm::opcodes;
use crate::codegen::CodegenCtx;
use javac_hir::hir::*;
use javac_ty::Ty;

pub fn gen_expr(mw: &mut MethodWriter, ctx: &mut CodegenCtx, expr: &Expr) {
    match expr {
        Expr::IntLiteral(v) => gen_int_const(mw, *v),
        Expr::LongLiteral(v) => gen_long_const(mw, *v),
        Expr::FloatLiteral(v) => {
            if *v == 0.0f32 { mw.visit_insn(opcodes::FCONST_0); }
            else if *v == 1.0f32 { mw.visit_insn(opcodes::FCONST_1); }
            else if *v == 2.0f32 { mw.visit_insn(opcodes::FCONST_2); }
            else { mw.visit_ldc_insn_float(*v); }
        }
        Expr::DoubleLiteral(v) => {
            if *v == 0.0 { mw.visit_insn(opcodes::DCONST_0); }
            else if *v == 1.0 { mw.visit_insn(opcodes::DCONST_1); }
            else { mw.visit_ldc_insn_double(*v); }
        }
        Expr::BoolLiteral(b) => mw.visit_insn(if *b { opcodes::ICONST_1 } else { opcodes::ICONST_0 }),
        Expr::NullLiteral => mw.visit_insn(opcodes::ACONST_NULL),
        Expr::StringLiteral(s) => mw.visit_ldc_insn_string(s),
        Expr::CharLiteral(c) => gen_int_const(mw, *c as i64),
        Expr::This => mw.visit_var_insn(opcodes::ALOAD, 0),
        Expr::Ident(name) => {
            if let Some(slot) = ctx.get_local(name) {
                if let Some(ty) = ctx.local_ty(name) {
                    mw.visit_var_insn(load_opcode(&ty), slot);
                }
            }
        }
        Expr::MethodCall { target, method, args } => {
            if let Some(t) = target {
                gen_expr(mw, ctx, t);
            }
            for arg in args {
                gen_expr(mw, ctx, arg);
            }
            let _ = method;
        }
        Expr::FieldAccess { target, field } => {
            gen_expr(mw, ctx, target);
            let _ = field;
        }
        Expr::Binary { op, left, right } => {
            gen_expr(mw, ctx, left);
            gen_expr(mw, ctx, right);
            gen_binary_op(mw, op, &left.ty());
        }
        Expr::Unary { op, operand } => {
            gen_expr(mw, ctx, operand);
            gen_unary_op(mw, op, &operand.ty());
        }
        Expr::NewObject { class, args } => {
            let name = class.internal_name();
            mw.visit_type_insn(opcodes::NEW, &name);
            mw.visit_insn(opcodes::DUP);
            for arg in args { gen_expr(mw, ctx, arg); }
            mw.visit_method_insn(opcodes::INVOKESPECIAL, &name, "<init>", "()V", false);
        }
        Expr::Parens(inner) => gen_expr(mw, ctx, inner),
        Expr::Cast { ty, expr: inner } => {
            gen_expr(mw, ctx, inner);
            gen_checkcast(mw, ty);
        }
        _ => {}
    }
}

fn gen_int_const(mw: &mut MethodWriter, v: i64) {
    match v {
        -1 => mw.visit_insn(opcodes::ICONST_M1),
        0 => mw.visit_insn(opcodes::ICONST_0),
        1 => mw.visit_insn(opcodes::ICONST_1),
        2 => mw.visit_insn(opcodes::ICONST_2),
        3 => mw.visit_insn(opcodes::ICONST_3),
        4 => mw.visit_insn(opcodes::ICONST_4),
        5 => mw.visit_insn(opcodes::ICONST_5),
        _ => mw.visit_ldc_insn_int(v as i32),
    }
}

fn gen_long_const(mw: &mut MethodWriter, v: i64) {
    match v {
        0 => mw.visit_insn(opcodes::LCONST_0),
        1 => mw.visit_insn(opcodes::LCONST_1),
        _ => mw.visit_ldc_insn_long(v),
    }
}

fn load_opcode(ty: &Ty) -> u8 {
    match ty {
        Ty::Int | Ty::Boolean | Ty::Byte | Ty::Char | Ty::Short => opcodes::ILOAD,
        Ty::Long => opcodes::LLOAD,
        Ty::Float => opcodes::FLOAD,
        Ty::Double => opcodes::DLOAD,
        _ => opcodes::ALOAD,
    }
}

fn gen_binary_op(mw: &mut MethodWriter, op: &BinaryOp, ty: &Ty) {
    match op {
        BinaryOp::Add => mw.visit_insn(add_opcode(ty)),
        BinaryOp::Sub => mw.visit_insn(sub_opcode(ty)),
        BinaryOp::Mul => mw.visit_insn(mul_opcode(ty)),
        BinaryOp::Div => mw.visit_insn(div_opcode(ty)),
        BinaryOp::Rem => mw.visit_insn(rem_opcode(ty)),
        BinaryOp::And => mw.visit_insn(if ty == &Ty::Long { opcodes::LAND } else { opcodes::IAND }),
        BinaryOp::Or => mw.visit_insn(if ty == &Ty::Long { opcodes::LOR } else { opcodes::IOR }),
        BinaryOp::Xor => mw.visit_insn(if ty == &Ty::Long { opcodes::LXOR } else { opcodes::IXOR }),
        BinaryOp::Shl => mw.visit_insn(if ty == &Ty::Long { opcodes::LSHL } else { opcodes::ISHL }),
        BinaryOp::Shr => mw.visit_insn(if ty == &Ty::Long { opcodes::LSHR } else { opcodes::ISHR }),
        BinaryOp::Ushr => mw.visit_insn(if ty == &Ty::Long { opcodes::LUSHR } else { opcodes::IUSHR }),
        _ => {}
    }
}

fn gen_unary_op(mw: &mut MethodWriter, op: &UnaryOp, ty: &Ty) {
    match op {
        UnaryOp::Neg => mw.visit_insn(neg_opcode(ty)),
        UnaryOp::Not => { mw.visit_insn(if ty == &Ty::Long { opcodes::LXOR } else if ty == &Ty::Int { opcodes::IXOR } else { opcodes::IXOR }); mw.visit_ldc_insn_int(-1); mw.visit_insn(if ty == &Ty::Long { opcodes::LXOR } else { opcodes::IXOR }); }
        UnaryOp::BitNot => { mw.visit_ldc_insn_int(-1); mw.visit_insn(if ty == &Ty::Long { opcodes::LXOR } else { opcodes::IXOR }); }
        UnaryOp::PreInc | UnaryOp::PreDec => {}
    }
}

fn gen_checkcast(mw: &mut MethodWriter, ty: &Ty) {
    match ty {
        Ty::Class(name) => mw.visit_type_insn(opcodes::CHECKCAST, name),
        Ty::Array(elem) => mw.visit_type_insn(opcodes::CHECKCAST, &elem.descriptor()),
        _ => {}
    }
}

fn add_opcode(ty: &Ty) -> u8 {
    match ty { Ty::Long => opcodes::LADD, Ty::Float => opcodes::FADD, Ty::Double => opcodes::DADD, _ => opcodes::IADD }
}
fn sub_opcode(ty: &Ty) -> u8 {
    match ty { Ty::Long => opcodes::LSUB, Ty::Float => opcodes::FSUB, Ty::Double => opcodes::DSUB, _ => opcodes::ISUB }
}
fn mul_opcode(ty: &Ty) -> u8 {
    match ty { Ty::Long => opcodes::LMUL, Ty::Float => opcodes::FMUL, Ty::Double => opcodes::DMUL, _ => opcodes::IMUL }
}
fn div_opcode(ty: &Ty) -> u8 {
    match ty { Ty::Long => opcodes::LDIV, Ty::Float => opcodes::FDIV, Ty::Double => opcodes::DDIV, _ => opcodes::IDIV }
}
fn rem_opcode(ty: &Ty) -> u8 {
    match ty { Ty::Long => opcodes::LREM, Ty::Float => opcodes::FREM, Ty::Double => opcodes::DREM, _ => opcodes::IREM }
}
fn neg_opcode(ty: &Ty) -> u8 {
    match ty { Ty::Long => opcodes::LNEG, Ty::Float => opcodes::FNEG, Ty::Double => opcodes::DNEG, _ => opcodes::INEG }
}