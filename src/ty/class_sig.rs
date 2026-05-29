use crate::ty::{Ty, TypeParam};
use ustr::Ustr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassSig {
    pub name: Ustr,
    pub super_class: Option<Ty>,
    pub interfaces: Vec<Ty>,
    pub type_params: Vec<TypeParam>,
    pub access_flags: u16,
}

impl ClassSig {
    pub fn new(name: Ustr) -> Self {
        Self {
            name,
            super_class: None,
            interfaces: Vec::new(),
            type_params: Vec::new(),
            access_flags: 0,
        }
    }

    pub fn internal_name(&self) -> String {
        self.name.as_str().replace('.', "/")
    }

    pub fn super_descriptor(&self) -> String {
        self.super_class
            .as_ref()
            .map(|t| t.descriptor())
            .unwrap_or_else(|| "Ljava/lang/Object;".to_string())
    }
}
