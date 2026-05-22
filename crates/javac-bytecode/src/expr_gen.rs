use crate::codegen::CodegenCtx;
use javac_classfile::{Label, MethodWriter};
use javac_hir::hir::*;
use javac_ty::Ty;
use rust_asm::opcodes;
use ustr::Ustr;

pub fn gen_expr(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, expr_id: ExprId) {
    let expr = &body.exprs[expr_id];
    match expr {
        Expr::IntLiteral(v) => gen_int_const(mw, *v),
        Expr::LongLiteral(v) => gen_long_const(mw, *v),
        Expr::FloatLiteral(v) => {
            if *v == 0.0f32 {
                mw.visit_insn(opcodes::FCONST_0);
            } else if *v == 1.0f32 {
                mw.visit_insn(opcodes::FCONST_1);
            } else if *v == 2.0f32 {
                mw.visit_insn(opcodes::FCONST_2);
            } else {
                mw.visit_ldc_insn_float(*v);
            }
        }
        Expr::DoubleLiteral(v) => {
            if *v == 0.0 {
                mw.visit_insn(opcodes::DCONST_0);
            } else if *v == 1.0 {
                mw.visit_insn(opcodes::DCONST_1);
            } else {
                mw.visit_ldc_insn_double(*v);
            }
        }
        Expr::BoolLiteral(b) => {
            mw.visit_insn(if *b {
                opcodes::ICONST_1
            } else {
                opcodes::ICONST_0
            });
        }
        Expr::NullLiteral => mw.visit_insn(opcodes::ACONST_NULL),
        Expr::StringLiteral(s) => mw.visit_ldc_insn_string(s),
        Expr::CharLiteral(c) => gen_int_const(mw, *c as i64),
        Expr::This => mw.visit_var_insn(opcodes::ALOAD, 0),
        Expr::Ident(name) => gen_ident(mw, ctx, *name),
        Expr::MethodCall {
            target,
            method,
            args,
        } => {
            if !gen_known_method_call(mw, ctx, body, *target, *method, args) {
                if let Some(target) = target {
                    discard_expr(mw, ctx, body, *target);
                }
                for arg in args {
                    discard_expr(mw, ctx, body, *arg);
                }
                let ty = expr_ty(ctx, body, expr_id);
                if !matches!(ty, Ty::Void) {
                    push_default_value(mw, &ty);
                }
            }
        }
        Expr::FieldAccess { target, field } => {
            if !gen_field_access(mw, ctx, body, *target, *field) {
                discard_expr(mw, ctx, body, *target);
                push_default_value(mw, &expr_ty(ctx, body, expr_id));
            }
        }
        Expr::Binary { op, left, right } => {
            gen_binary_expr(mw, ctx, body, op.clone(), *left, *right)
        }
        Expr::Unary { op, operand } => gen_unary_expr(mw, ctx, body, op, *operand),
        Expr::NewObject { class, args } => {
            let name = class.internal_name();
            mw.visit_type_insn(opcodes::NEW, &name);
            mw.visit_insn(opcodes::DUP);
            let mut desc = String::from("(");
            for arg in args {
                gen_expr(mw, ctx, body, *arg);
                desc.push_str(&expr_ty(ctx, body, *arg).erasure().descriptor());
            }
            desc.push_str(")V");
            mw.visit_method_insn(opcodes::INVOKESPECIAL, &name, "<init>", &desc, false);
        }
        Expr::Parens(inner) => gen_expr(mw, ctx, body, *inner),
        Expr::Cast { ty, expr: inner } => {
            gen_expr(mw, ctx, body, *inner);
            coerce(mw, &expr_ty(ctx, body, *inner), ty);
        }
        Expr::Assign { target, op, value } => gen_assign(mw, ctx, body, *target, op, *value),
        Expr::PostInc(inner) => gen_post_inc_dec(mw, ctx, body, *inner, 1),
        Expr::PostDec(inner) => gen_post_inc_dec(mw, ctx, body, *inner, -1),
        _ => push_default_value(mw, &expr_ty(ctx, body, expr_id)),
    }
}

pub fn expr_ty(ctx: &CodegenCtx, body: &Body, expr_id: ExprId) -> Ty {
    match &body.exprs[expr_id] {
        Expr::Ident(name) => ctx
            .local_ty(*name)
            .or_else(|| ctx.field_ty(*name))
            .unwrap_or(Ty::Int),
        Expr::FieldAccess { target, field } => {
            if is_system_out(body, *target, *field) {
                Ty::Class(Ustr::from("java/io/PrintStream"))
            } else if is_current_instance(body, *target) {
                ctx.field_ty(*field).unwrap_or(Ty::Int)
            } else {
                body.exprs[expr_id].ty(&body.exprs)
            }
        }
        Expr::MethodCall { target, method, .. } => {
            known_method_return_ty(ctx, body, *target, *method)
                .unwrap_or_else(|| body.exprs[expr_id].ty(&body.exprs))
        }
        Expr::Binary { op, left, right } => match op {
            BinaryOp::AndAnd
            | BinaryOp::OrOr
            | BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::Le
            | BinaryOp::Ge => Ty::Boolean,
            BinaryOp::Add
                if is_string_ty(&expr_ty(ctx, body, *left))
                    || is_string_ty(&expr_ty(ctx, body, *right)) =>
            {
                Ty::Class(Ustr::from("java/lang/String"))
            }
            _ => expr_ty(ctx, body, *left),
        },
        Expr::Unary { op, operand } => match op {
            UnaryOp::Not => Ty::Boolean,
            _ => expr_ty(ctx, body, *operand),
        },
        Expr::Assign { value, .. } => expr_ty(ctx, body, *value),
        Expr::Parens(inner) => expr_ty(ctx, body, *inner),
        Expr::Cast { ty, .. } => ty.clone(),
        _ => body.exprs[expr_id].ty(&body.exprs),
    }
}

pub fn coerce(mw: &mut MethodWriter, from: &Ty, to: &Ty) {
    if from == to {
        return;
    }

    match (from.erasure(), to.erasure()) {
        (Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean, Ty::Long) => {
            mw.visit_insn(opcodes::I2L);
        }
        (Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean, Ty::Float) => {
            mw.visit_insn(opcodes::I2F);
        }
        (Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean, Ty::Double) => {
            mw.visit_insn(opcodes::I2D);
        }
        (Ty::Long, Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean) => {
            mw.visit_insn(opcodes::L2I);
        }
        (Ty::Long, Ty::Float) => mw.visit_insn(opcodes::L2F),
        (Ty::Long, Ty::Double) => mw.visit_insn(opcodes::L2D),
        (Ty::Float, Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean) => {
            mw.visit_insn(opcodes::F2I);
        }
        (Ty::Float, Ty::Long) => mw.visit_insn(opcodes::F2L),
        (Ty::Float, Ty::Double) => mw.visit_insn(opcodes::F2D),
        (Ty::Double, Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean) => {
            mw.visit_insn(opcodes::D2I);
        }
        (Ty::Double, Ty::Long) => mw.visit_insn(opcodes::D2L),
        (Ty::Double, Ty::Float) => mw.visit_insn(opcodes::D2F),
        (_, Ty::Byte) => mw.visit_insn(opcodes::I2B),
        (_, Ty::Char) => mw.visit_insn(opcodes::I2C),
        (_, Ty::Short) => mw.visit_insn(opcodes::I2S),
        (_, Ty::Class(name)) => mw.visit_type_insn(opcodes::CHECKCAST, name.as_str()),
        (_, Ty::Array(elem)) => mw.visit_type_insn(opcodes::CHECKCAST, &elem.descriptor()),
        _ => {}
    }
}

pub fn pop_ty(mw: &mut MethodWriter, ty: &Ty) {
    if matches!(ty, Ty::Void) {
        return;
    }
    if ty.size() == 2 {
        mw.visit_insn(opcodes::POP2);
    } else {
        mw.visit_insn(opcodes::POP);
    }
}

pub fn push_default_value(mw: &mut MethodWriter, ty: &Ty) {
    match ty {
        Ty::Void => {}
        Ty::Long => mw.visit_insn(opcodes::LCONST_0),
        Ty::Float => mw.visit_insn(opcodes::FCONST_0),
        Ty::Double => mw.visit_insn(opcodes::DCONST_0),
        Ty::Class(_) | Ty::Array(_) | Ty::TypeVar(_) | Ty::Wildcard(_) | Ty::Intersection(_) => {
            mw.visit_insn(opcodes::ACONST_NULL);
        }
        _ => mw.visit_insn(opcodes::ICONST_0),
    }
}

fn gen_ident(mw: &mut MethodWriter, ctx: &mut CodegenCtx, name: Ustr) {
    if let Some(slot) = ctx.get_local(name) {
        if let Some(ty) = ctx.local_ty(name) {
            mw.visit_var_insn(load_opcode(&ty), slot);
        }
    } else if let Some(ty) = ctx.field_ty(name) {
        mw.visit_var_insn(opcodes::ALOAD, 0);
        mw.visit_field_insn(
            opcodes::GETFIELD,
            ctx.class_name.as_str(),
            name.as_str(),
            &ty.descriptor(),
        );
    } else {
        push_default_value(mw, &Ty::Int);
    }
}

fn gen_field_access(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    field: Ustr,
) -> bool {
    if is_system_out(body, target, field) {
        mw.visit_field_insn(
            opcodes::GETSTATIC,
            "java/lang/System",
            "out",
            "Ljava/io/PrintStream;",
        );
        return true;
    }

    if is_current_instance(body, target) {
        if let Some(ty) = ctx.field_ty(field) {
            mw.visit_var_insn(opcodes::ALOAD, 0);
            mw.visit_field_insn(
                opcodes::GETFIELD,
                ctx.class_name.as_str(),
                field.as_str(),
                &ty.descriptor(),
            );
            return true;
        }
    }

    false
}

fn gen_known_method_call(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: Option<ExprId>,
    method: Ustr,
    args: &[ExprId],
) -> bool {
    if method.as_str() == "println" && target.is_some_and(|target| is_system_out_expr(body, target))
    {
        mw.visit_field_insn(
            opcodes::GETSTATIC,
            "java/lang/System",
            "out",
            "Ljava/io/PrintStream;",
        );
        let mut desc = String::from("(");
        for arg in args {
            gen_expr(mw, ctx, body, *arg);
            desc.push_str(&expr_ty(ctx, body, *arg).descriptor());
        }
        desc.push_str(")V");
        mw.visit_method_insn(
            opcodes::INVOKEVIRTUAL,
            "java/io/PrintStream",
            "println",
            &desc,
            false,
        );
        return true;
    }

    if let Some(target) = target {
        let target_ty = expr_ty(ctx, body, target).erasure();
        if target_ty == Ty::Class(Ustr::from("java/lang/String")) {
            if method.as_str() == "length" && args.is_empty() {
                gen_expr(mw, ctx, body, target);
                mw.visit_method_insn(
                    opcodes::INVOKEVIRTUAL,
                    "java/lang/String",
                    "length",
                    "()I",
                    false,
                );
                return true;
            }
            if method.as_str() == "charAt" && args.len() == 1 {
                gen_expr(mw, ctx, body, target);
                gen_expr(mw, ctx, body, args[0]);
                mw.visit_method_insn(
                    opcodes::INVOKEVIRTUAL,
                    "java/lang/String",
                    "charAt",
                    "(I)C",
                    false,
                );
                return true;
            }
        }
        if is_current_instance(body, target) {
            if let Some(sig) = ctx.method_sig(method) {
                gen_expr(mw, ctx, body, target);
                for arg in args {
                    gen_expr(mw, ctx, body, *arg);
                }
                mw.visit_method_insn(
                    opcodes::INVOKEVIRTUAL,
                    ctx.class_name.as_str(),
                    method.as_str(),
                    &sig.descriptor(),
                    false,
                );
                return true;
            }
        }
    } else if let Some(sig) = ctx.method_sig(method) {
        let is_static = sig.access_flags & javac_classfile::ACC_STATIC != 0;
        if !is_static {
            mw.visit_var_insn(opcodes::ALOAD, 0);
        }
        for arg in args {
            gen_expr(mw, ctx, body, *arg);
        }
        mw.visit_method_insn(
            if is_static {
                opcodes::INVOKESTATIC
            } else {
                opcodes::INVOKEVIRTUAL
            },
            ctx.class_name.as_str(),
            method.as_str(),
            &sig.descriptor(),
            false,
        );
        return true;
    }

    false
}

fn known_method_return_ty(
    ctx: &CodegenCtx,
    body: &Body,
    target: Option<ExprId>,
    method: Ustr,
) -> Option<Ty> {
    if method.as_str() == "println" && target.is_some_and(|target| is_system_out_expr(body, target))
    {
        return Some(Ty::Void);
    }
    if let Some(target) = target {
        let target_ty = expr_ty(ctx, body, target).erasure();
        if target_ty == Ty::Class(Ustr::from("java/lang/String")) {
            return match method.as_str() {
                "length" => Some(Ty::Int),
                "charAt" => Some(Ty::Char),
                _ => None,
            };
        }
    }
    ctx.method_sig(method).map(|sig| sig.return_type)
}

fn gen_binary_expr(
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
        BinaryOp::AndAnd => gen_short_circuit_and(mw, ctx, body, left, right),
        BinaryOp::OrOr => gen_short_circuit_or(mw, ctx, body, left, right),
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
            gen_expr(mw, ctx, body, left);
            gen_expr(mw, ctx, body, right);
            gen_comparison(mw, &op, &left_ty);
        }
        BinaryOp::Add if is_string_ty(&left_ty) || is_string_ty(&right_ty) => {
            discard_expr(mw, ctx, body, left);
            discard_expr(mw, ctx, body, right);
            mw.visit_ldc_insn_string("");
        }
        _ => {
            gen_expr(mw, ctx, body, left);
            gen_expr(mw, ctx, body, right);
            gen_binary_op(mw, &op, &left_ty);
        }
    }
}

fn gen_short_circuit_and(
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

fn gen_short_circuit_or(
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

fn gen_comparison(mw: &mut MethodWriter, op: &BinaryOp, ty: &Ty) {
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

fn gen_unary_expr(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    op: &UnaryOp,
    operand: ExprId,
) {
    match op {
        UnaryOp::PreInc => gen_pre_inc_dec(mw, ctx, body, operand, 1),
        UnaryOp::PreDec => gen_pre_inc_dec(mw, ctx, body, operand, -1),
        _ => {
            gen_expr(mw, ctx, body, operand);
            match op {
                UnaryOp::Neg => mw.visit_insn(neg_opcode(&expr_ty(ctx, body, operand))),
                UnaryOp::Not => {
                    mw.visit_insn(opcodes::ICONST_1);
                    mw.visit_insn(opcodes::IXOR);
                }
                UnaryOp::BitNot => {
                    let ty = expr_ty(ctx, body, operand);
                    if ty == Ty::Long {
                        gen_long_const(mw, -1);
                        mw.visit_insn(opcodes::LXOR);
                    } else {
                        mw.visit_insn(opcodes::ICONST_M1);
                        mw.visit_insn(opcodes::IXOR);
                    }
                }
                UnaryOp::PreInc | UnaryOp::PreDec => {}
            }
        }
    }
}

fn gen_assign(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    op: &AssignOp,
    value: ExprId,
) {
    if let Expr::Ident(name) = &body.exprs[target] {
        if let Some(slot) = ctx.get_local(*name) {
            let ty = ctx.local_ty(*name).unwrap_or(Ty::Int);
            if !matches!(op, AssignOp::Plain) {
                mw.visit_var_insn(load_opcode(&ty), slot);
            }
            gen_expr(mw, ctx, body, value);
            coerce(mw, &expr_ty(ctx, body, value), &ty);
            if !matches!(op, AssignOp::Plain) {
                gen_assign_op(mw, op, &ty);
            }
            dup_ty(mw, &ty);
            mw.visit_var_insn(crate::local_var::store_opcode(&ty), slot);
            return;
        }
    }

    gen_expr(mw, ctx, body, value);
}

fn gen_pre_inc_dec(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    amount: i16,
) {
    if let Expr::Ident(name) = &body.exprs[target] {
        if let Some(slot) = ctx.get_local(*name) {
            mw.visit_iinc_insn(slot, amount);
            let ty = ctx.local_ty(*name).unwrap_or(Ty::Int);
            mw.visit_var_insn(load_opcode(&ty), slot);
            return;
        }
    }
    push_default_value(mw, &expr_ty(ctx, body, target));
}

fn gen_post_inc_dec(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    target: ExprId,
    amount: i16,
) {
    if let Expr::Ident(name) = &body.exprs[target] {
        if let Some(slot) = ctx.get_local(*name) {
            let ty = ctx.local_ty(*name).unwrap_or(Ty::Int);
            mw.visit_var_insn(load_opcode(&ty), slot);
            mw.visit_iinc_insn(slot, amount);
            return;
        }
    }
    push_default_value(mw, &expr_ty(ctx, body, target));
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

fn discard_expr(mw: &mut MethodWriter, ctx: &mut CodegenCtx, body: &Body, expr_id: ExprId) {
    gen_expr(mw, ctx, body, expr_id);
    pop_ty(mw, &expr_ty(ctx, body, expr_id));
}

fn dup_ty(mw: &mut MethodWriter, ty: &Ty) {
    if ty.size() == 2 {
        mw.visit_insn(opcodes::DUP2);
    } else {
        mw.visit_insn(opcodes::DUP);
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
        _ => {}
    }
}

fn gen_assign_op(mw: &mut MethodWriter, op: &AssignOp, ty: &Ty) {
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

fn is_current_instance(body: &Body, expr_id: ExprId) -> bool {
    matches!(body.exprs[expr_id], Expr::This)
}

fn is_system_out(body: &Body, target: ExprId, field: Ustr) -> bool {
    field.as_str() == "out"
        && matches!(&body.exprs[target], Expr::Ident(name) if name.as_str() == "System")
}

fn is_system_out_expr(body: &Body, expr_id: ExprId) -> bool {
    matches!(&body.exprs[expr_id], Expr::FieldAccess { target, field } if is_system_out(body, *target, *field))
}

fn is_string_ty(ty: &Ty) -> bool {
    matches!(ty.erasure(), Ty::Class(name) if name.as_str() == "java/lang/String")
}
