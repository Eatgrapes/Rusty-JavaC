use crate::ty::{Ty, TypeParam};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodSig {
    pub name: String,
    pub params: Vec<Ty>,
    pub return_type: Ty,
    pub type_params: Vec<TypeParam>,
    pub access_flags: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldSig {
    pub name: String,
    pub ty: Ty,
    pub access_flags: u16,
}

impl MethodSig {
    pub fn new(name: String, params: Vec<Ty>, return_type: Ty) -> Self {
        Self {
            name,
            params,
            return_type,
            type_params: Vec::new(),
            access_flags: 0,
        }
    }

    pub fn descriptor(&self) -> String {
        let mut desc = String::from("(");
        for p in &self.params {
            desc.push_str(&p.erasure().descriptor());
        }
        desc.push(')');
        desc.push_str(&self.return_type.erasure().descriptor());
        desc
    }

    pub fn param_count(&self) -> usize {
        self.params.len()
    }

    pub fn param_slots(&self) -> usize {
        self.params.iter().map(|t| t.erasure().size()).sum()
    }
}