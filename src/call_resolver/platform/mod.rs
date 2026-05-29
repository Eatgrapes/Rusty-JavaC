mod java_io;
mod java_lang;
mod java_math;
mod java_net;
mod java_nio_file;
mod java_time;
mod java_util;
mod java_util_function;

const ACC_PUBLIC: u16 = 0x0001;
const ACC_STATIC: u16 = 0x0008;

#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub owner: &'static str,
    pub name: &'static str,
    pub descriptor: &'static str,
    pub access_flags: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Method {
    pub owner: &'static str,
    pub name: &'static str,
    pub descriptor: &'static str,
    pub access_flags: u16,
    pub is_interface: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Parent {
    pub owner: &'static str,
    pub parent: &'static str,
}

pub(crate) const fn public_static_field(
    owner: &'static str,
    name: &'static str,
    descriptor: &'static str,
) -> Field {
    Field {
        owner,
        name,
        descriptor,
        access_flags: ACC_PUBLIC | ACC_STATIC,
    }
}

pub(crate) const fn public_instance_method(
    owner: &'static str,
    name: &'static str,
    descriptor: &'static str,
) -> Method {
    public_method(owner, name, descriptor, false)
}

pub(crate) const fn public_interface_method(
    owner: &'static str,
    name: &'static str,
    descriptor: &'static str,
) -> Method {
    public_method(owner, name, descriptor, true)
}

const fn public_method(
    owner: &'static str,
    name: &'static str,
    descriptor: &'static str,
    is_interface: bool,
) -> Method {
    Method {
        owner,
        name,
        descriptor,
        access_flags: ACC_PUBLIC,
        is_interface,
    }
}

pub(crate) const fn parent(owner: &'static str, parent: &'static str) -> Parent {
    Parent { owner, parent }
}

const PACKAGE_CLASSES: &[&[&str]] = &[
    java_io::CLASSES,
    java_lang::CLASSES,
    java_math::CLASSES,
    java_net::CLASSES,
    java_nio_file::CLASSES,
    java_time::CLASSES,
    java_util::CLASSES,
    java_util_function::CLASSES,
];

const PACKAGE_INTERFACES: &[&[&str]] = &[
    java_io::INTERFACES,
    java_util::INTERFACES,
    java_util_function::INTERFACES,
];

const PACKAGE_FIELDS: &[&[Field]] = &[java_lang::FIELDS];

const PACKAGE_METHODS: &[&[Method]] = &[
    java_io::METHODS,
    java_lang::METHODS,
    java_util::METHODS,
    java_util_function::METHODS,
];

const PACKAGE_PARENTS: &[&[Parent]] = &[
    java_io::PARENTS,
    java_lang::PARENTS,
    java_util::PARENTS,
    java_util_function::PARENTS,
];

pub fn classes() -> impl Iterator<Item = &'static str> {
    PACKAGE_CLASSES
        .iter()
        .flat_map(|classes| classes.iter().copied())
}

pub fn interfaces() -> impl Iterator<Item = &'static str> {
    PACKAGE_INTERFACES
        .iter()
        .flat_map(|interfaces| interfaces.iter().copied())
}

pub fn fields() -> impl Iterator<Item = Field> {
    PACKAGE_FIELDS
        .iter()
        .flat_map(|fields| fields.iter().copied())
}

pub fn methods() -> impl Iterator<Item = Method> {
    PACKAGE_METHODS
        .iter()
        .flat_map(|methods| methods.iter().copied())
}

pub fn parents() -> impl Iterator<Item = Parent> {
    PACKAGE_PARENTS
        .iter()
        .flat_map(|parents| parents.iter().copied())
}

pub fn class_name(simple_name: &str) -> Option<&'static str> {
    classes().find(|name| simple_name_of(name) == simple_name)
}

pub fn internal_class_name(internal_name: &str) -> Option<&'static str> {
    classes().find(|name| *name == internal_name)
}

pub fn package_name(package: &str) -> bool {
    classes().any(|name| package_of(name) == package)
}

fn simple_name_of(internal_name: &str) -> &str {
    internal_name.rsplit('/').next().unwrap_or(internal_name)
}

fn package_of(internal_name: &str) -> &str {
    internal_name
        .rsplit_once('/')
        .map_or("", |(package, _)| package)
}
