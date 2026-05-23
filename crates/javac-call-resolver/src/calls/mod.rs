mod java_io;
mod java_lang;
mod system;

use crate::{FieldRef, MethodRef};
use javac_ty::Ty;

pub(super) fn resolve_static_field(owner: &str, name: &str) -> Option<FieldRef> {
    system::resolve_static_field(owner, name)
}

pub(super) fn resolve_instance_method(receiver: &Ty, name: &str, args: &[Ty]) -> Option<MethodRef> {
    java_lang::resolve_instance_method(receiver, name, args)
        .or_else(|| java_io::resolve_instance_method(receiver, name, args))
        .or_else(|| system::resolve_instance_method(receiver, name, args))
}
