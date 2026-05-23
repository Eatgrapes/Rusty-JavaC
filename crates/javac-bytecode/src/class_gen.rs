use crate::codegen::CodegenCtx;
use javac_classfile::ClassFileWriter;
use javac_hir::hir::*;
use javac_ty::Ty;
use rust_asm::opcodes;

const JAVA_VERSION: u32 = 21;
const OBJECT_CLASS: &str = "java/lang/Object";
const INIT_METHOD: &str = "<init>";

pub fn gen_class(unit: &CompilationUnit) -> Result<Vec<u8>, String> {
    let type_decl = unit
        .type_decls
        .first()
        .ok_or_else(|| "no type declarations".to_string())?;
    let mut writer = ClassFileWriter::new();
    gen_type_decl(&mut writer, type_decl);
    writer.to_bytes()
}

fn gen_type_decl(writer: &mut ClassFileWriter, type_decl: &TypeDecl) {
    let access_flags = class_access_flags(type_decl);
    let super_name = super_name(type_decl);
    let interfaces = interface_names(type_decl);
    let interface_refs: Vec<_> = interfaces.iter().map(String::as_str).collect();

    writer.visit(
        JAVA_VERSION,
        access_flags,
        &type_decl.name,
        Some(super_name.as_str()),
        &interface_refs,
    );
    if let Some(signature) = &type_decl.generic_signature {
        writer.visit_signature(signature);
    }

    gen_fields(writer, &type_decl.fields);
    if needs_default_constructor(type_decl) {
        gen_default_constructor(writer, &super_name);
    }
    gen_methods(writer, type_decl, &super_name);
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

fn gen_methods(writer: &mut ClassFileWriter, type_decl: &TypeDecl, super_name: &str) {
    for method in &type_decl.methods {
        gen_method(writer, type_decl, method, super_name);
    }
}

fn gen_method(
    writer: &mut ClassFileWriter,
    type_decl: &TypeDecl,
    method: &MethodDecl,
    super_name: &str,
) {
    let descriptor = method.signature.descriptor();
    let mut mw = writer.visit_method(method.access_flags, &method.name, &descriptor);
    if let Some(signature) = &method.generic_signature {
        mw.visit_signature(signature);
    }

    if method_has_code(method)
        && let Some(block) = &method.root_block
    {
        mw.visit_code();
        let mut ctx = CodegenCtx::new(writer, type_decl.name.clone());
        ctx.set_super_name(ustr::Ustr::from(super_name));
        ctx.set_fields(&type_decl.fields);
        ctx.set_methods(&type_decl.methods);
        ctx.begin_method(method);
        declare_method_locals(&mut mw, type_decl, method);
        gen_constructor_prelude(&mut mw, &ctx, method);
        crate::method_gen::gen_method_body(&mut mw, &mut ctx, &method.body, block);
        mw.visit_maxs(0, 0);
    }

    mw.visit_end(writer);
}

fn declare_method_locals(
    mw: &mut javac_classfile::MethodWriter,
    type_decl: &TypeDecl,
    method: &MethodDecl,
) {
    let mut slot = 0;
    if method.access_flags & javac_classfile::ACC_STATIC == 0 {
        mw.visit_local_variable("this", &Ty::Class(type_decl.name).descriptor(), slot);
        slot += 1;
    }

    for param in &method.params {
        mw.visit_local_variable(param.name.as_str(), &param.ty.erasure().descriptor(), slot);
        slot += param.ty.size() as u16;
    }
}

fn gen_constructor_prelude(
    mw: &mut javac_classfile::MethodWriter,
    ctx: &CodegenCtx,
    method: &MethodDecl,
) {
    if method.name != INIT_METHOD {
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
    method.access_flags & (javac_classfile::ACC_ABSTRACT | javac_classfile::ACC_NATIVE) == 0
}

fn class_access_flags(type_decl: &TypeDecl) -> u16 {
    if matches!(type_decl.kind, TypeDeclKind::Class) {
        type_decl.access_flags | javac_classfile::ACC_SUPER
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

fn gen_default_constructor(writer: &mut ClassFileWriter, super_name: &str) {
    let mut mw = writer.visit_method(javac_classfile::ACC_PUBLIC, INIT_METHOD, "()V");
    mw.visit_code();
    mw.visit_var_insn(opcodes::ALOAD, 0);
    mw.visit_method_insn(
        opcodes::INVOKESPECIAL,
        super_name,
        INIT_METHOD,
        "()V",
        false,
    );
    mw.visit_insn(opcodes::RETURN);
    mw.visit_maxs(0, 0);
    mw.visit_end(writer);
}
