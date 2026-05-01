use javac_ty::Ty;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Scope {
    vars: HashMap<String, Ty>,
    parent: Option<Box<Scope>>,
}

impl Scope {
    pub fn new() -> Self {
        Self { vars: HashMap::new(), parent: None }
    }

    pub fn child(&self) -> Self {
        Self { vars: HashMap::new(), parent: Some(Box::new(self.clone())) }
    }

    pub fn define(&mut self, name: String, ty: Ty) {
        self.vars.insert(name, ty);
    }

    pub fn resolve(&self, name: &str) -> Option<&Ty> {
        self.vars.get(name).or_else(|| self.parent.as_ref()?.resolve(name))
    }
}