use crate::classfile::MethodWriter;
use rust_asm::opcodes;

pub(super) fn emit_int(mw: &mut MethodWriter, value: i64) {
    match value {
        -1 => mw.visit_insn(opcodes::ICONST_M1),
        0 => mw.visit_insn(opcodes::ICONST_0),
        1 => mw.visit_insn(opcodes::ICONST_1),
        2 => mw.visit_insn(opcodes::ICONST_2),
        3 => mw.visit_insn(opcodes::ICONST_3),
        4 => mw.visit_insn(opcodes::ICONST_4),
        5 => mw.visit_insn(opcodes::ICONST_5),
        _ => mw.visit_ldc_insn_int(value as i32),
    }
}

pub(super) fn emit_long(mw: &mut MethodWriter, value: i64) {
    match value {
        0 => mw.visit_insn(opcodes::LCONST_0),
        1 => mw.visit_insn(opcodes::LCONST_1),
        _ => mw.visit_ldc_insn_long(value),
    }
}

pub(super) fn emit_float(mw: &mut MethodWriter, value: f32) {
    if value == 0.0 {
        mw.visit_insn(opcodes::FCONST_0);
    } else if value == 1.0 {
        mw.visit_insn(opcodes::FCONST_1);
    } else if value == 2.0 {
        mw.visit_insn(opcodes::FCONST_2);
    } else {
        mw.visit_ldc_insn_float(value);
    }
}

pub(super) fn emit_double(mw: &mut MethodWriter, value: f64) {
    if value == 0.0 {
        mw.visit_insn(opcodes::DCONST_0);
    } else if value == 1.0 {
        mw.visit_insn(opcodes::DCONST_1);
    } else {
        mw.visit_ldc_insn_double(value);
    }
}

pub(super) fn emit_bool(mw: &mut MethodWriter, value: bool) {
    mw.visit_insn(if value {
        opcodes::ICONST_1
    } else {
        opcodes::ICONST_0
    });
}
