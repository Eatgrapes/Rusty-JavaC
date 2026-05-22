use crate::{FieldRef, MethodRef};
use javac_ty::Ty;

pub fn resolve_static_field(_owner: &str, _name: &str) -> Option<FieldRef> {
    None
}

pub fn resolve_instance_method(_receiver: &Ty, _name: &str, _args: &[Ty]) -> Option<MethodRef> {
    None
}
