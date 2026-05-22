use crate::codegen::CodegenCtx;
use javac_classfile::ClassFileWriter;
use javac_hir::hir::*;
use rust_asm::opcodes;

pub fn gen_class(unit: &CompilationUnit) -> Result<Vec<u8>, String> {
    for td in &unit.type_decls {
        let mut writer = ClassFileWriter::new();
        let access = if matches!(td.kind, TypeDeclKind::Class) {
            td.access_flags | javac_classfile::ACC_SUPER
        } else {
            td.access_flags
        };
        let super_name = td.super_class.as_ref().map(|t| t.internal_name());
        let interfaces: Vec<String> = td.interfaces.iter().map(|t| t.internal_name()).collect();
        let iface_refs: Vec<&str> = interfaces.iter().map(|s| s.as_str()).collect();

        writer.visit(21, access, &td.name, super_name.as_deref(), &iface_refs);

        for field in &td.fields {
            let desc = field.ty.descriptor();
            writer
                .visit_field(field.access_flags, &field.name, &desc)
                .visit_end(&mut writer);
        }

        if matches!(td.kind, TypeDeclKind::Class) && !td.methods.iter().any(|m| m.name == "<init>")
        {
            gen_default_constructor(
                &mut writer,
                super_name.as_deref().unwrap_or("java/lang/Object"),
            );
        }

        for method in &td.methods {
            let desc = method.signature.descriptor();
            let mut mw = writer.visit_method(method.access_flags, &method.name, &desc);
            let has_no_code = method.access_flags
                & (javac_classfile::ACC_ABSTRACT | javac_classfile::ACC_NATIVE)
                != 0;
            if !has_no_code && let Some(block) = &method.root_block {
                mw.visit_code();
                let mut ctx = CodegenCtx::new(&mut writer, td.name.clone());
                ctx.set_super_name(ustr::Ustr::from(
                    super_name.as_deref().unwrap_or("java/lang/Object"),
                ));
                ctx.set_fields(&td.fields);
                ctx.set_methods(&td.methods);
                ctx.begin_method(method);
                if method.name == "<init>" {
                    mw.visit_var_insn(opcodes::ALOAD, 0);
                    mw.visit_method_insn(
                        opcodes::INVOKESPECIAL,
                        ctx.super_name.as_str(),
                        "<init>",
                        "()V",
                        false,
                    );
                }
                crate::method_gen::gen_method_body(&mut mw, &mut ctx, &method.body, block);
                mw.visit_maxs(0, 0);
            }
            mw.visit_end(&mut writer);
        }

        return writer.to_bytes();
    }
    Err("no type declarations".to_string())
}

fn gen_default_constructor(writer: &mut ClassFileWriter, super_name: &str) {
    let mut mw = writer.visit_method(javac_classfile::ACC_PUBLIC, "<init>", "()V");
    mw.visit_code();
    mw.visit_var_insn(opcodes::ALOAD, 0);
    mw.visit_method_insn(opcodes::INVOKESPECIAL, super_name, "<init>", "()V", false);
    mw.visit_insn(opcodes::RETURN);
    mw.visit_maxs(0, 0);
    mw.visit_end(writer);
}
