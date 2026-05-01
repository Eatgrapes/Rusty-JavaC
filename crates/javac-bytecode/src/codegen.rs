use javac_classfile::ClassFileWriter;
use javac_ty::Ty;
use std::collections::HashMap;
use ustr::Ustr;

pub struct CodegenCtx<'a> {
    pub writer: &'a mut ClassFileWriter,
    pub class_name: Ustr,
    pub next_local: u16,
    pub locals: HashMap<Ustr, u16>,
    pub local_types: HashMap<Ustr, Ty>,
}

impl<'a> CodegenCtx<'a> {
    pub fn new(writer: &'a mut ClassFileWriter, class_name: Ustr) -> Self {
        Self {
            writer,
            class_name,
            next_local: 0,
            locals: HashMap::new(),
            local_types: HashMap::new(),
        }
    }

    pub fn alloc_local(&mut self, name: Ustr, ty: Ty) -> u16 {
        let slot = self.next_local;
        self.locals.insert(name, slot);
        self.local_types.insert(name, ty.clone());
        self.next_local += ty.size() as u16;
        slot
    }

    pub fn get_local(&self, name: Ustr) -> Option<u16> {
        self.locals.get(&name).copied()
    }

    pub fn local_ty(&self, name: Ustr) -> Option<Ty> {
        self.local_types.get(&name).cloned()
    }
}
