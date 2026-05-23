use javac_classfile::MethodWriter;
use javac_ty::Ty;
use rust_asm::opcodes;

pub(crate) fn coerce(mw: &mut MethodWriter, from: &Ty, to: &Ty) {
    if from == to {
        return;
    }

    emit_numeric_conversion(mw, from, to);
}

pub(crate) fn cast(mw: &mut MethodWriter, from: &Ty, to: &Ty) {
    if from == to {
        return;
    }

    if emit_numeric_conversion(mw, from, to) {
        return;
    }

    if let Some(name) = checkcast_target(to) {
        mw.visit_type_insn(opcodes::CHECKCAST, &name);
    }
}

fn emit_numeric_conversion(mw: &mut MethodWriter, from: &Ty, to: &Ty) -> bool {
    match (from.erasure(), to.erasure()) {
        (Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean, Ty::Long) => {
            mw.visit_insn(opcodes::I2L);
            true
        }
        (Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean, Ty::Float) => {
            mw.visit_insn(opcodes::I2F);
            true
        }
        (Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean, Ty::Double) => {
            mw.visit_insn(opcodes::I2D);
            true
        }
        (Ty::Long, Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean) => {
            mw.visit_insn(opcodes::L2I);
            true
        }
        (Ty::Long, Ty::Float) => {
            mw.visit_insn(opcodes::L2F);
            true
        }
        (Ty::Long, Ty::Double) => {
            mw.visit_insn(opcodes::L2D);
            true
        }
        (Ty::Float, Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean) => {
            mw.visit_insn(opcodes::F2I);
            true
        }
        (Ty::Float, Ty::Long) => {
            mw.visit_insn(opcodes::F2L);
            true
        }
        (Ty::Float, Ty::Double) => {
            mw.visit_insn(opcodes::F2D);
            true
        }
        (Ty::Double, Ty::Int | Ty::Byte | Ty::Short | Ty::Char | Ty::Boolean) => {
            mw.visit_insn(opcodes::D2I);
            true
        }
        (Ty::Double, Ty::Long) => {
            mw.visit_insn(opcodes::D2L);
            true
        }
        (Ty::Double, Ty::Float) => {
            mw.visit_insn(opcodes::D2F);
            true
        }
        (_, Ty::Byte) => {
            mw.visit_insn(opcodes::I2B);
            true
        }
        (_, Ty::Char) => {
            mw.visit_insn(opcodes::I2C);
            true
        }
        (_, Ty::Short) => {
            mw.visit_insn(opcodes::I2S);
            true
        }
        _ => false,
    }
}

fn checkcast_target(ty: &Ty) -> Option<String> {
    match ty.erasure() {
        Ty::Class(name) => Some(name.to_string()),
        Ty::Array(_) => Some(ty.erasure().descriptor()),
        _ => None,
    }
}

pub(crate) fn pop_ty(mw: &mut MethodWriter, ty: &Ty) {
    if matches!(ty, Ty::Void) {
        return;
    }

    mw.visit_insn(if ty.size() == 2 {
        opcodes::POP2
    } else {
        opcodes::POP
    });
}

pub(crate) fn push_default_value(mw: &mut MethodWriter, ty: &Ty) {
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

pub(super) fn dup_ty(mw: &mut MethodWriter, ty: &Ty) {
    mw.visit_insn(if ty.size() == 2 {
        opcodes::DUP2
    } else {
        opcodes::DUP
    });
}
