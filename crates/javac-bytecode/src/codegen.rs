use javac_classfile::{ClassFileWriter, Label};
use javac_hir::hir::{FieldDecl, MethodDecl};
use javac_ty::{MethodSig, Ty};
use std::collections::HashMap;
use ustr::Ustr;

pub struct CodegenCtx<'a> {
    pub writer: &'a mut ClassFileWriter,
    pub class_name: Ustr,
    pub super_name: Ustr,
    pub return_ty: Ty,
    pub next_local: u16,
    pub locals: HashMap<Ustr, u16>,
    pub local_types: HashMap<Ustr, Ty>,
    pub fields: HashMap<Ustr, Ty>,
    pub methods: HashMap<Ustr, MethodSig>,
    pub break_labels: Vec<Label>,
    pub continue_labels: Vec<Label>,
}

impl<'a> CodegenCtx<'a> {
    pub fn new(writer: &'a mut ClassFileWriter, class_name: Ustr) -> Self {
        Self {
            writer,
            class_name,
            super_name: Ustr::from("java/lang/Object"),
            return_ty: Ty::Void,
            next_local: 0,
            locals: HashMap::new(),
            local_types: HashMap::new(),
            fields: HashMap::new(),
            methods: HashMap::new(),
            break_labels: Vec::new(),
            continue_labels: Vec::new(),
        }
    }

    pub fn set_super_name(&mut self, super_name: Ustr) {
        self.super_name = super_name;
    }

    pub fn set_fields(&mut self, fields: &[FieldDecl]) {
        self.fields.clear();
        for field in fields {
            self.fields.insert(field.name, field.ty.clone());
        }
    }

    pub fn set_methods(&mut self, methods: &[MethodDecl]) {
        self.methods.clear();
        for method in methods {
            let mut sig = method.signature.clone();
            sig.access_flags = method.access_flags;
            self.methods.insert(method.name, sig);
        }
    }

    pub fn begin_method(&mut self, method: &MethodDecl) {
        self.return_ty = method.signature.return_type.clone();
        self.next_local = 0;
        self.locals.clear();
        self.local_types.clear();

        if method.access_flags & javac_classfile::ACC_STATIC == 0 {
            self.next_local = 1;
        }

        for param in &method.params {
            let slot = self.next_local;
            self.locals.insert(param.name, slot);
            self.local_types.insert(param.name, param.ty.clone());
            self.next_local += param.ty.size() as u16;
        }
    }

    pub fn alloc_local(&mut self, name: Ustr, ty: Ty) -> u16 {
        let slot = self.next_local;
        self.locals.insert(name, slot);
        self.local_types.insert(name, ty.clone());
        self.next_local += ty.size() as u16;
        slot
    }

    pub fn alloc_temp(&mut self, ty: &Ty) -> u16 {
        let slot = self.next_local;
        self.next_local += ty.size() as u16;
        slot
    }

    pub fn get_local(&self, name: Ustr) -> Option<u16> {
        self.locals.get(&name).copied()
    }

    pub fn local_ty(&self, name: Ustr) -> Option<Ty> {
        self.local_types.get(&name).cloned()
    }

    pub fn field_ty(&self, name: Ustr) -> Option<Ty> {
        self.fields.get(&name).cloned()
    }

    pub fn method_sig(&self, name: Ustr) -> Option<MethodSig> {
        self.methods.get(&name).cloned()
    }
}
