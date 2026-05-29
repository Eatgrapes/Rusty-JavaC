use crate::bytecode::codegen::CodegenCtx;
use crate::bytecode::expr_gen::{coerce, expr_ty, gen_expr, literals};
use crate::classfile::MethodWriter;
use crate::hir::{ArrayInit, Body, ExprId};
use crate::ty::Ty;
use rust_asm::opcodes;

pub(super) fn emit_new_array(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    element_type: &Ty,
    dimensions: &[Option<ExprId>],
    initializer: Option<&ArrayInit>,
) {
    if element_type.is_primitive() {
        emit_array_length(mw, ctx, body, dimensions, initializer);
        mw.visit_new_array(primitive_array_type_code(element_type));
    } else {
        emit_array_length(mw, ctx, body, dimensions, initializer);
        mw.visit_type_insn(opcodes::ANEWARRAY, &element_type.internal_name());
    }

    if let Some(initializer) = initializer {
        emit_array_initializer(mw, ctx, body, element_type, initializer);
    }
}

pub(super) fn emit_array_access(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    array: ExprId,
    index: ExprId,
) {
    let element_type = array_element_type(ctx, body, array);
    gen_expr(mw, ctx, body, array);
    gen_expr(mw, ctx, body, index);
    coerce(mw, &expr_ty(ctx, body, index), &Ty::Int);
    mw.visit_insn(array_load_opcode(&element_type));
}

pub(super) fn array_store_opcode(element_type: &Ty) -> u8 {
    match element_type.erasure() {
        Ty::Long => opcodes::LASTORE,
        Ty::Float => opcodes::FASTORE,
        Ty::Double => opcodes::DASTORE,
        Ty::Class(_) | Ty::Array(_) | Ty::TypeVar(_) | Ty::Wildcard(_) | Ty::Intersection(_) => {
            opcodes::AASTORE
        }
        Ty::Byte | Ty::Boolean => opcodes::BASTORE,
        Ty::Char => opcodes::CASTORE,
        Ty::Short => opcodes::SASTORE,
        _ => opcodes::IASTORE,
    }
}

pub(super) fn array_element_type(ctx: &CodegenCtx, body: &Body, array: ExprId) -> Ty {
    match expr_ty(ctx, body, array).erasure() {
        Ty::Array(element) => *element,
        _ => Ty::Int,
    }
}

fn emit_array_length(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    dimensions: &[Option<ExprId>],
    initializer: Option<&ArrayInit>,
) {
    if let Some(Some(size)) = dimensions.first() {
        gen_expr(mw, ctx, body, *size);
        coerce(mw, &expr_ty(ctx, body, *size), &Ty::Int);
    } else {
        let len = initializer
            .map(|init| init.elements.len())
            .unwrap_or_default();
        literals::emit_int(mw, len as i64);
    }
}

fn emit_array_initializer(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    element_type: &Ty,
    initializer: &ArrayInit,
) {
    for (index, element) in initializer.elements.iter().copied().enumerate() {
        mw.visit_insn(opcodes::DUP);
        literals::emit_int(mw, index as i64);
        gen_expr(mw, ctx, body, element);
        coerce(mw, &expr_ty(ctx, body, element), element_type);
        mw.visit_insn(array_store_opcode(element_type));
    }
}

pub(super) fn primitive_array_type_code(ty: &Ty) -> u8 {
    match ty {
        Ty::Boolean => 4,
        Ty::Char => 5,
        Ty::Float => 6,
        Ty::Double => 7,
        Ty::Byte => 8,
        Ty::Short => 9,
        Ty::Int => 10,
        Ty::Long => 11,
        _ => 10,
    }
}

pub(crate) fn array_load_opcode(element_type: &Ty) -> u8 {
    match element_type.erasure() {
        Ty::Long => opcodes::LALOAD,
        Ty::Float => opcodes::FALOAD,
        Ty::Double => opcodes::DALOAD,
        Ty::Class(_) | Ty::Array(_) | Ty::TypeVar(_) | Ty::Wildcard(_) | Ty::Intersection(_) => {
            opcodes::AALOAD
        }
        Ty::Byte | Ty::Boolean => opcodes::BALOAD,
        Ty::Char => opcodes::CALOAD,
        Ty::Short => opcodes::SALOAD,
        _ => opcodes::IALOAD,
    }
}
