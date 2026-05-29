use crate::bytecode::lambda::LambdaTable;
use crate::call_resolver::ClassCatalog;
use crate::classfile::{ClassFileWriter, Label};
use crate::hir::{AnonymousClassInfo, Block, CapturedField, FieldDecl, MethodDecl, OuterThisInfo};
use crate::ty::{MethodSig, Ty};
use std::collections::HashMap;
use ustr::Ustr;

#[derive(Clone)]
pub struct FieldInfo {
    pub ty: Ty,
    pub access_flags: u16,
}

#[derive(Clone)]
pub struct CleanupResource {
    pub ty: Ty,
    pub slot: u16,
}

#[derive(Clone)]
pub struct CleanupScope {
    pub resources: Vec<CleanupResource>,
    pub finally: Option<Block>,
}

#[derive(Clone, Copy)]
pub struct ControlTarget {
    pub label: Label,
    pub cleanup_depth: usize,
}

pub struct CodegenCtx<'a> {
    pub writer: &'a mut ClassFileWriter,
    pub catalog: ClassCatalog,
    pub class_name: Ustr,
    pub super_name: Ustr,
    pub return_ty: Ty,
    pub next_local: u16,
    pub locals: HashMap<Ustr, u16>,
    pub local_types: HashMap<Ustr, Ty>,
    pub fields: HashMap<Ustr, FieldInfo>,
    pub methods: HashMap<Ustr, MethodSig>,
    pub outer_this: Option<OuterThisInfo>,
    pub outer_fields: HashMap<Ustr, CapturedField>,
    pub enclosing_static_owner: Option<Ustr>,
    pub break_labels: Vec<ControlTarget>,
    pub continue_labels: Vec<ControlTarget>,
    pub labeled_break_labels: Vec<(Ustr, ControlTarget)>,
    pub labeled_continue_labels: Vec<(Ustr, ControlTarget)>,
    pub cleanup_scopes: Vec<CleanupScope>,
    pub(crate) lambdas: LambdaTable,
}

impl<'a> CodegenCtx<'a> {
    pub fn new(writer: &'a mut ClassFileWriter, class_name: Ustr, catalog: &ClassCatalog) -> Self {
        Self {
            writer,
            catalog: catalog.clone(),
            class_name,
            super_name: Ustr::from("java/lang/Object"),
            return_ty: Ty::Void,
            next_local: 0,
            locals: HashMap::new(),
            local_types: HashMap::new(),
            fields: HashMap::new(),
            methods: HashMap::new(),
            outer_this: None,
            outer_fields: HashMap::new(),
            enclosing_static_owner: None,
            break_labels: Vec::new(),
            continue_labels: Vec::new(),
            labeled_break_labels: Vec::new(),
            labeled_continue_labels: Vec::new(),
            cleanup_scopes: Vec::new(),
            lambdas: HashMap::new(),
        }
    }

    pub fn set_super_name(&mut self, super_name: Ustr) {
        self.super_name = super_name;
    }

    pub fn set_fields(&mut self, fields: &[FieldDecl]) {
        self.fields.clear();
        for field in fields {
            self.fields.insert(
                field.name,
                FieldInfo {
                    ty: field.ty.clone(),
                    access_flags: field.access_flags,
                },
            );
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

    pub fn set_anonymous_info(&mut self, info: Option<&AnonymousClassInfo>) {
        self.outer_this = info.and_then(|info| info.outer_this.clone());
        self.enclosing_static_owner = info.and_then(|info| info.enclosing_static_owner);
        self.outer_fields.clear();
        if let Some(info) = info {
            for field in &info.outer_fields {
                self.outer_fields.insert(field.name, field.clone());
            }
        }
    }

    pub fn begin_method(&mut self, method: &MethodDecl) {
        self.return_ty = method.signature.return_type.clone();
        self.next_local = 0;
        self.locals.clear();
        self.local_types.clear();

        if method.access_flags & crate::classfile::ACC_STATIC == 0 {
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
        self.fields
            .get(&name)
            .map(|field| field.ty.clone())
            .or_else(|| self.outer_fields.get(&name).map(|field| field.ty.clone()))
            .or_else(|| {
                self.enclosing_static_owner.and_then(|owner| {
                    self.catalog
                        .resolve_static_field(owner.as_str(), name.as_str())
                        .map(|field| field.ty)
                })
            })
    }

    pub fn field_is_static(&self, name: Ustr) -> bool {
        self.fields
            .get(&name)
            .is_some_and(|field| field.access_flags & crate::classfile::ACC_STATIC != 0)
    }

    pub fn method_sig(&self, name: Ustr) -> Option<MethodSig> {
        self.methods.get(&name).cloned()
    }

    pub fn control_target(&self, label: Label) -> ControlTarget {
        ControlTarget {
            label,
            cleanup_depth: self.cleanup_scopes.len(),
        }
    }

    pub fn push_labeled_loop(
        &mut self,
        label: Ustr,
        break_label: ControlTarget,
        continue_label: ControlTarget,
    ) {
        self.labeled_break_labels.push((label, break_label));
        self.labeled_continue_labels.push((label, continue_label));
    }

    pub fn pop_labeled_loop(&mut self) {
        self.labeled_break_labels.pop();
        self.labeled_continue_labels.pop();
    }

    pub fn find_break_target(&self, label: Option<Ustr>) -> Option<ControlTarget> {
        match label {
            Some(label) => self
                .labeled_break_labels
                .iter()
                .rev()
                .find_map(|(name, target)| (*name == label).then_some(*target)),
            None => self.break_labels.last().copied(),
        }
    }

    pub fn find_continue_target(&self, label: Option<Ustr>) -> Option<ControlTarget> {
        match label {
            Some(label) => self
                .labeled_continue_labels
                .iter()
                .rev()
                .find_map(|(name, target)| (*name == label).then_some(*target)),
            None => self.continue_labels.last().copied(),
        }
    }
}
