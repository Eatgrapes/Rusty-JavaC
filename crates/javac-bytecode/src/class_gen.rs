use crate::codegen::CodegenCtx;
use javac_classfile::ClassFileWriter;
use javac_hir::hir::*;

pub fn gen_class(unit: &CompilationUnit) -> Result<Vec<u8>, String> {
    for td in &unit.type_decls {
        let mut writer = ClassFileWriter::new();
        let access = td.access_flags;
        let super_name = td.super_class.as_ref().map(|t| t.internal_name());
        let interfaces: Vec<String> = td.interfaces.iter().map(|t| t.internal_name()).collect();
        let iface_refs: Vec<&str> = interfaces.iter().map(|s| s.as_str()).collect();

        writer.visit(21, access, &td.name, super_name.as_deref(), &iface_refs);

        for field in &td.fields {
            let desc = field.ty.descriptor();
            writer.visit_field(field.access_flags, &field.name, &desc).visit_end(&mut writer);
        }

        for method in &td.methods {
            let desc = method.signature.descriptor();
            let mut mw = writer.visit_method(method.access_flags, &method.name, &desc);
            if let Some(block) = &method.root_block {
                mw.visit_code();
                let mut ctx = CodegenCtx::new(&mut writer, td.name.clone());
                crate::method_gen::gen_method_body(&mut mw, &mut ctx, &method.body, block);
                mw.visit_maxs(0, 0);
            }
            mw.visit_end(&mut writer);
        }

        return writer.to_bytes();
    }
    Err("no type declarations".to_string())
}