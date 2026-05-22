pub mod java_lang;
pub mod javax;
pub mod system;

use javac_ty::Ty;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRef {
    pub owner: &'static str,
    pub name: &'static str,
    pub descriptor: &'static str,
    pub ty: Ty,
    pub is_static: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodRef {
    pub owner: &'static str,
    pub name: &'static str,
    pub descriptor: String,
    pub return_ty: Ty,
    pub opcode: u8,
    pub is_interface: bool,
}

pub fn resolve_static_field(owner: &str, name: &str) -> Option<FieldRef> {
    system::resolve_static_field(owner, name)
        .or_else(|| java_lang::resolve_static_field(owner, name))
        .or_else(|| javax::resolve_static_field(owner, name))
}

pub fn resolve_instance_method(receiver: &Ty, name: &str, args: &[Ty]) -> Option<MethodRef> {
    java_lang::resolve_instance_method(receiver, name, args)
        .or_else(|| system::resolve_instance_method(receiver, name, args))
        .or_else(|| javax::resolve_instance_method(receiver, name, args))
}
