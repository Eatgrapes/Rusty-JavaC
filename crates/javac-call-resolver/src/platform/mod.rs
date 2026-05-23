mod java_io;
mod java_lang;
mod java_math;
mod java_net;
mod java_nio_file;
mod java_time;
mod java_util;
mod java_util_function;

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

pub fn classes() -> impl Iterator<Item = &'static str> {
    PACKAGE_CLASSES
        .iter()
        .flat_map(|classes| classes.iter().copied())
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
