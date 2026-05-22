use javac_classfile::MethodWriter;
use javac_ty::Ty;
use rust_asm::opcodes;

pub(crate) fn coerce(mw: &mut MethodWriter, from: &Ty, to: &Ty) {
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
