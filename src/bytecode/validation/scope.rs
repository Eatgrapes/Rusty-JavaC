use crate::ty::Ty;
use std::collections::HashMap;
use ustr::Ustr;

#[derive(Clone, Default)]
pub(super) struct MethodScope {
    pub locals: HashMap<Ustr, Ty>,
    pub line: Option<u16>,
}

impl MethodScope {
    pub fn with_line(&self, line: Option<u16>) -> Self {
        let mut scope = self.clone();
        scope.line = line;
        scope
    }
}
