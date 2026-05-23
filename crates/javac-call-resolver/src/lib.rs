mod calls;
mod catalog;
mod platform;

pub use catalog::ClassCatalog;
use javac_ty::Ty;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRef {
    pub owner: &'static str,
    pub name: &'static str,
    pub descriptor: &'static str,
    pub ty: Ty,
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

pub fn resolve_class_name(simple_name: &str) -> Option<&'static str> {
    platform::class_name(simple_name)
}

pub fn resolve_internal_class_name(internal_name: &str) -> Option<&'static str> {
    platform::internal_class_name(internal_name)
}

pub fn resolve_import(path: &str, is_wildcard: bool) -> Option<&'static str> {
    let internal_name = path.replace('.', "/");
    if is_wildcard {
        return known_package(internal_name.as_str()).then_some("");
    }
    resolve_internal_class_name(internal_name.as_str())
}

pub fn known_package(package: &str) -> bool {
    platform::package_name(package)
}

pub fn resolve_static_field(owner: &str, name: &str) -> Option<FieldRef> {
    calls::resolve_static_field(owner, name)
}

pub fn resolve_instance_method(receiver: &Ty, name: &str, args: &[Ty]) -> Option<MethodRef> {
    calls::resolve_instance_method(receiver, name, args)
}
