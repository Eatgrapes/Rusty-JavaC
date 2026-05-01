use javac_classfile::ClassFileWriter;
use javac_ty::Ty;
use std::collections::HashMap;

pub struct CodegenCtx<'a> {
    pub writer: &'a mut ClassFileWriter,
    pub class_name: String,
    pub next_local: u16,
    pub locals: HashMap<String, u16>,
    pub local_types: HashMap<String, Ty>,
}

impl<'a> CodegenCtx<'a> {
    pub fn new(writer: &'a mut ClassFileWriter, class_name: String) -> Self {
        Self {
            writer,
            class_name,
            next_local: 0,
            locals: HashMap::new(),
            local_types: HashMap::new(),
        }
    }

    pub fn alloc_local(&mut self, name: &str, ty: Ty) -> u16 {
        let slot = self.next_local;
        self.locals.insert(name.to_string(), slot);
        self.local_types.insert(name.to_string(), ty.clone());
        self.next_local += ty.size() as u16;
        slot
    }

    pub fn get_local(&self, name: &str) -> Option<u16> {
        self.locals.get(name).copied()
    }

    pub fn resolve_local_ty(&self, name: &str) -> Option<Ty> {
        self.local_types.get(name).cloned()
    }
}