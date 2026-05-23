use crate::codegen::CodegenCtx;
use crate::expr_gen::{coerce, expr_ty, gen_expr, literals};
use javac_classfile::MethodWriter;
use javac_hir::hir::{ArrayInit, Body, ExprId};
use javac_ty::Ty;
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
        emit_primitive_array(mw, ctx, body, element_type, dimensions, initializer);
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

fn emit_primitive_array(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    body: &Body,
    element_type: &Ty,
    dimensions: &[Option<ExprId>],
    initializer: Option<&ArrayInit>,
) {
    emit_primitive_class(mw, element_type);
    emit_array_length(mw, ctx, body, dimensions, initializer);
    mw.visit_method_insn(
        opcodes::INVOKESTATIC,
        "java/lang/reflect/Array",
        "newInstance",
        "(Ljava/lang/Class;I)Ljava/lang/Object;",
        false,
    );
    mw.visit_type_insn(
        opcodes::CHECKCAST,
        &Ty::Array(Box::new(element_type.clone())).descriptor(),
    );
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

fn emit_primitive_class(mw: &mut MethodWriter, ty: &Ty) {
    let owner = match ty {
        Ty::Boolean => "java/lang/Boolean",
        Ty::Byte => "java/lang/Byte",
        Ty::Char => "java/lang/Character",
        Ty::Short => "java/lang/Short",
        Ty::Int => "java/lang/Integer",
        Ty::Long => "java/lang/Long",
        Ty::Float => "java/lang/Float",
        Ty::Double => "java/lang/Double",
        _ => "java/lang/Object",
    };
    mw.visit_field_insn(opcodes::GETSTATIC, owner, "TYPE", "Ljava/lang/Class;");
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
