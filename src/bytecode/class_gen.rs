use crate::bytecode::codegen::CodegenCtx;
use crate::bytecode::error::BytecodeError;
use crate::bytecode::lambda::{self, LambdaTable};
use crate::call_resolver::ClassCatalog;
use crate::classfile::ClassFileWriter;
use crate::hir::*;
use crate::ty::Ty;
use rust_asm::constants::V21;
use rust_asm::opcodes;

const OBJECT_CLASS: &str = "java/lang/Object";
const INIT_METHOD: &str = "<init>";
const CLINIT_METHOD: &str = "<clinit>";

pub struct GeneratedClass {
    pub internal_name: String,
    pub bytes: Vec<u8>,
}

pub fn gen_class(unit: &CompilationUnit) -> Result<Vec<u8>, BytecodeError> {
    let catalog = ClassCatalog::platform();
    gen_class_with_catalog(unit, &catalog)
}

pub fn gen_class_with_catalog(
    unit: &CompilationUnit,
    catalog: &ClassCatalog,
) -> Result<Vec<u8>, BytecodeError> {
    gen_class_with_source_file(unit, catalog, None)
}

pub fn gen_class_with_source_file(
    unit: &CompilationUnit,
    catalog: &ClassCatalog,
    source_file: Option<&str>,
) -> Result<Vec<u8>, BytecodeError> {
    gen_classes_with_source_file(unit, catalog, source_file)?
        .into_iter()
        .next()
        .map(|class| class.bytes)
        .ok_or_else(|| BytecodeError::new("no type declarations"))
}

pub fn gen_classes_with_source_file(
    unit: &CompilationUnit,
    catalog: &ClassCatalog,
    source_file: Option<&str>,
) -> Result<Vec<GeneratedClass>, BytecodeError> {
    let type_decl = unit
        .type_decls
        .first()
        .ok_or_else(|| BytecodeError::new("no type declarations"))?;
    let mut classes = Vec::new();
    let nest_members = nested_type_names(type_decl);
    gen_type_decl_class(
        type_decl,
        catalog,
        source_file,
        None,
        &nest_members,
        type_decl.name.as_str(),
        &mut classes,
    )?;
    Ok(classes)
}

fn gen_type_decl_class(
    type_decl: &TypeDecl,
    catalog: &ClassCatalog,
    source_file: Option<&str>,
    nest_host: Option<&str>,
    nest_members: &[String],
    root_nest_host: &str,
    classes: &mut Vec<GeneratedClass>,
) -> Result<(), BytecodeError> {
    crate::bytecode::validation::validate_type_decl(type_decl, catalog)?;
    let mut writer = ClassFileWriter::new();
    if let Some(source_file) = source_file {
        writer.visit_source_file(source_file);
    }
    if let Some(host) = nest_host {
        writer.visit_nest_host(host);
    } else {
        for member in nest_members {
            writer.visit_nest_member(member);
        }
    }
    gen_type_decl(&mut writer, type_decl, catalog);
    classes.push(GeneratedClass {
        internal_name: type_decl.name.to_string(),
        bytes: writer.to_bytes().map_err(BytecodeError::new)?,
    });
    for inner in &type_decl.inner_types {
        gen_type_decl_class(
            inner,
            catalog,
            source_file,
            Some(root_nest_host),
            &[],
            root_nest_host,
            classes,
        )?;
    }
    Ok(())
}

fn nested_type_names(type_decl: &TypeDecl) -> Vec<String> {
    let mut names = Vec::new();
    collect_nested_type_names(type_decl, &mut names);
    names
}

fn collect_nested_type_names(type_decl: &TypeDecl, names: &mut Vec<String>) {
    for inner in &type_decl.inner_types {
        names.push(inner.name.to_string());
        collect_nested_type_names(inner, names);
    }
}

fn gen_type_decl(writer: &mut ClassFileWriter, type_decl: &TypeDecl, catalog: &ClassCatalog) {
    let access_flags = class_access_flags(type_decl);
    let super_name = super_name(type_decl);
    let interfaces = interface_names(type_decl);
    let interface_refs: Vec<_> = interfaces.iter().map(String::as_str).collect();

    writer.visit(
        V21,
        access_flags,
        &type_decl.name,
        Some(super_name.as_str()),
        &interface_refs,
    );
    if let Some(signature) = &type_decl.generic_signature {
        writer.visit_signature(signature);
    }

    gen_fields(writer, &type_decl.fields);
    gen_static_initializer(writer, type_decl, catalog);
    if needs_default_constructor(type_decl) {
        gen_default_constructor(writer, type_decl, &super_name, catalog);
    }

    let mut counter = 0u32;
    for method in &type_decl.methods {
        let lambdas = lambda::emit_lambda_methods(
            writer,
            type_decl,
            &super_name,
            catalog,
            method,
            &mut counter,
        );
        gen_method(writer, type_decl, method, &super_name, catalog, &lambdas);
    }
}

fn gen_fields(writer: &mut ClassFileWriter, fields: &[FieldDecl]) {
    for field in fields {
        let descriptor = field.ty.descriptor();
        let mut fw = writer.visit_field(field.access_flags, &field.name, &descriptor);
        if let Some(signature) = &field.generic_signature {
            fw.visit_signature(signature);
        }
        fw.visit_end(writer);
    }
}

fn gen_method(
    writer: &mut ClassFileWriter,
    type_decl: &TypeDecl,
    method: &MethodDecl,
    super_name: &str,
    catalog: &ClassCatalog,
    lambdas: &LambdaTable,
) {
    let descriptor = method.signature.descriptor();
    let mut mw = writer.visit_method(method.access_flags, &method.name, &descriptor);
    if let Some(signature) = &method.generic_signature {
        mw.visit_signature(signature);
    }
    for exception in &method.throws {
        mw.visit_exception(&exception.internal_name());
    }

    if method_has_code(method)
        && let Some(block) = &method.root_block
    {
        mw.visit_code();
        let mut ctx = CodegenCtx::new(writer, type_decl.name, catalog);
        ctx.set_super_name(ustr::Ustr::from(super_name));
        ctx.set_fields(&type_decl.fields);
        ctx.set_methods(&type_decl.methods);
        ctx.set_anonymous_info(type_decl.anonymous.as_ref());
        ctx.lambdas = lambdas.clone();
        ctx.begin_method(method);
        declare_method_locals(&mut mw, type_decl, method);
        gen_constructor_prelude(&mut mw, &ctx, method);
        if method.name == INIT_METHOD {
            emit_instance_field_initializers(&mut mw, &mut ctx, &type_decl.fields);
        }
        crate::bytecode::method_gen::gen_method_body(&mut mw, &mut ctx, &method.body, block);
        mw.visit_maxs(0, 0);
    }

    mw.visit_end(writer);
}

fn gen_static_initializer(
    writer: &mut ClassFileWriter,
    type_decl: &TypeDecl,
    catalog: &ClassCatalog,
) {
    if !has_static_field_initializers(&type_decl.fields) {
        return;
    }

    let mut mw = writer.visit_method(crate::classfile::ACC_STATIC, CLINIT_METHOD, "()V");
    mw.visit_code();
    let mut ctx = CodegenCtx::new(writer, type_decl.name, catalog);
    ctx.set_fields(&type_decl.fields);
    ctx.set_methods(&type_decl.methods);
    ctx.set_anonymous_info(type_decl.anonymous.as_ref());
    emit_static_field_initializers(&mut mw, &mut ctx, &type_decl.fields);
    mw.visit_insn(opcodes::RETURN);
    mw.visit_maxs(0, 0);
    mw.visit_end(writer);
}

fn declare_method_locals(
    mw: &mut crate::classfile::MethodWriter,
    type_decl: &TypeDecl,
    method: &MethodDecl,
) {
    let mut slot = 0;
    if method.access_flags & crate::classfile::ACC_STATIC == 0 {
        mw.visit_local_variable("this", &Ty::Class(type_decl.name).descriptor(), slot);
        slot += 1;
    }

    for param in &method.params {
        mw.visit_local_variable(param.name.as_str(), &param.ty.erasure().descriptor(), slot);
        slot += param.ty.size() as u16;
    }
}

fn gen_constructor_prelude(
    mw: &mut crate::classfile::MethodWriter,
    ctx: &CodegenCtx,
    method: &MethodDecl,
) {
    if method.name != INIT_METHOD {
        return;
    }

    if let Some(call) = &method.constructor_call {
        mw.visit_var_insn(opcodes::ALOAD, 0);
        let mut slot = 1u16;
        for (index, param) in method.params.iter().enumerate() {
            if index >= call.arg_offset {
                mw.visit_var_insn(crate::bytecode::local_var::load_opcode(&param.ty), slot);
            }
            slot += param.ty.size() as u16;
        }
        let descriptor = format!(
            "({})V",
            call.params
                .iter()
                .map(|ty| ty.erasure().descriptor())
                .collect::<String>()
        );
        mw.visit_method_insn(
            opcodes::INVOKESPECIAL,
            &call.owner.internal_name(),
            INIT_METHOD,
            &descriptor,
            false,
        );
        if let Some(outer_this) = &ctx.outer_this {
            mw.visit_var_insn(opcodes::ALOAD, 0);
            mw.visit_var_insn(opcodes::ALOAD, 1);
            mw.visit_field_insn(
                opcodes::PUTFIELD,
                ctx.class_name.as_str(),
                outer_this.field_name.as_str(),
                &outer_this.ty.descriptor(),
            );
        }
        return;
    }

    mw.visit_var_insn(opcodes::ALOAD, 0);
    mw.visit_method_insn(
        opcodes::INVOKESPECIAL,
        ctx.super_name.as_str(),
        INIT_METHOD,
        "()V",
        false,
    );
}

fn method_has_code(method: &MethodDecl) -> bool {
    method.access_flags & (crate::classfile::ACC_ABSTRACT | crate::classfile::ACC_NATIVE) == 0
}

fn class_access_flags(type_decl: &TypeDecl) -> u16 {
    if matches!(type_decl.kind, TypeDeclKind::Class) {
        type_decl.access_flags | crate::classfile::ACC_SUPER
    } else {
        type_decl.access_flags
    }
}

fn super_name(type_decl: &TypeDecl) -> String {
    type_decl
        .super_class
        .as_ref()
        .map(|ty| ty.internal_name())
        .unwrap_or_else(|| OBJECT_CLASS.to_string())
}

fn interface_names(type_decl: &TypeDecl) -> Vec<String> {
    type_decl
        .interfaces
        .iter()
        .map(|ty| ty.internal_name())
        .collect()
}

fn needs_default_constructor(type_decl: &TypeDecl) -> bool {
    matches!(type_decl.kind, TypeDeclKind::Class)
        && !type_decl
            .methods
            .iter()
            .any(|method| method.name == INIT_METHOD)
}

fn has_static_field_initializers(fields: &[FieldDecl]) -> bool {
    fields.iter().any(|field| {
        field.access_flags & crate::classfile::ACC_STATIC != 0 && field.initializer.is_some()
    })
}

fn gen_default_constructor(
    writer: &mut ClassFileWriter,
    type_decl: &TypeDecl,
    super_name: &str,
    catalog: &ClassCatalog,
) {
    let mut mw = writer.visit_method(crate::classfile::ACC_PUBLIC, INIT_METHOD, "()V");
    mw.visit_code();
    let mut ctx = CodegenCtx::new(writer, type_decl.name, catalog);
    ctx.set_super_name(ustr::Ustr::from(super_name));
    ctx.set_fields(&type_decl.fields);
    ctx.set_methods(&type_decl.methods);
    ctx.set_anonymous_info(type_decl.anonymous.as_ref());
    mw.visit_var_insn(opcodes::ALOAD, 0);
    mw.visit_method_insn(
        opcodes::INVOKESPECIAL,
        super_name,
        INIT_METHOD,
        "()V",
        false,
    );
    emit_instance_field_initializers(&mut mw, &mut ctx, &type_decl.fields);
    mw.visit_insn(opcodes::RETURN);
    mw.visit_maxs(0, 0);
    mw.visit_end(writer);
}

fn emit_instance_field_initializers(
    mw: &mut crate::classfile::MethodWriter,
    ctx: &mut CodegenCtx,
    fields: &[FieldDecl],
) {
    for field in fields {
        if field.access_flags & crate::classfile::ACC_STATIC != 0 {
            continue;
        }
        let Some(initializer) = field.initializer else {
            continue;
        };

        mw.visit_var_insn(opcodes::ALOAD, 0);
        crate::bytecode::expr_gen::gen_expr(mw, ctx, &field.body, initializer);
        let value_ty = crate::bytecode::expr_gen::expr_ty(ctx, &field.body, initializer);
        crate::bytecode::expr_gen::coerce(mw, &value_ty, &field.ty);
        mw.visit_field_insn(
            opcodes::PUTFIELD,
            ctx.class_name.as_str(),
            field.name.as_str(),
            &field.ty.descriptor(),
        );
    }
}

fn emit_static_field_initializers(
    mw: &mut crate::classfile::MethodWriter,
    ctx: &mut CodegenCtx,
    fields: &[FieldDecl],
) {
    for field in fields {
        if field.access_flags & crate::classfile::ACC_STATIC == 0 {
            continue;
        }
        let Some(initializer) = field.initializer else {
            continue;
        };

        crate::bytecode::expr_gen::gen_expr(mw, ctx, &field.body, initializer);
        let value_ty = crate::bytecode::expr_gen::expr_ty(ctx, &field.body, initializer);
        crate::bytecode::expr_gen::coerce(mw, &value_ty, &field.ty);
        mw.visit_field_insn(
            opcodes::PUTSTATIC,
            ctx.class_name.as_str(),
            field.name.as_str(),
            &field.ty.descriptor(),
        );
    }
}
