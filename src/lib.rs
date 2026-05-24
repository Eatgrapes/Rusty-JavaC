extern crate self as javac_ast;
extern crate self as javac_bytecode;
extern crate self as javac_call_resolver;
extern crate self as javac_classfile;
extern crate self as javac_compiler;
extern crate self as javac_diagnostics;
extern crate self as javac_hir;
extern crate self as javac_lexer;
extern crate self as javac_parser;
extern crate self as javac_ty;

include!("../crates/javac-ast/src/lib.rs");

#[path = "../crates/javac-ty/src/check.rs"]
pub mod check;
#[path = "../crates/javac-ty/src/class_sig.rs"]
pub mod class_sig;
#[path = "../crates/javac-ty/src/descriptor.rs"]
pub mod descriptor;
#[path = "../crates/javac-ty/src/erasure.rs"]
pub mod erasure;
#[path = "../crates/javac-ty/src/method_sig.rs"]
pub mod method_sig;
#[path = "../crates/javac-ty/src/ty.rs"]
pub mod ty;

pub use class_sig::ClassSig;
pub use method_sig::{FieldSig, MethodSig};
pub use ty::{Ty, TypeParam};

#[allow(unused_imports)]
#[path = "../crates/javac-classfile/src/lib.rs"]
mod classfile_impl;
#[path = "../crates/javac-diagnostics/src/lib.rs"]
mod diagnostics_impl;
#[path = "../crates/javac-lexer/src/lib.rs"]
mod lexer_impl;

pub use classfile_impl::access_flags::*;
pub use classfile_impl::{ClassFileWriter, FieldWriter, Label, MethodWriter};
pub use classfile_impl::{access_flags, constant_pool, reader, writer};
pub use diagnostics_impl::{
    Diagnostic, Diagnostics, Result as DiagnosticResult, Severity, SourceFile, render_diagnostic,
    render_diagnostics,
};
pub use lexer_impl::{LexedToken, Lexer, RawToken, raw_to_syntax};

#[path = "../crates/javac-parser/src/parser/mod.rs"]
pub mod parser;

pub use parser::{Parse, ParseError, Parser};

#[path = "../crates/javac-hir/src/hir.rs"]
pub mod hir;
#[path = "../crates/javac-hir/src/item.rs"]
pub mod item;
#[path = "../crates/javac-hir/src/lowering.rs"]
pub mod lowering;
#[path = "../crates/javac-hir/src/name_resolver.rs"]
pub mod name_resolver;

#[path = "../crates/javac-call-resolver/src/catalog.rs"]
mod catalog;
#[path = "../crates/javac-call-resolver/src/platform/mod.rs"]
mod platform;

pub use catalog::ClassCatalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRef {
    pub owner: String,
    pub name: String,
    pub descriptor: String,
    pub ty: Ty,
    pub access_flags: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodRef {
    pub owner: String,
    pub name: String,
    pub descriptor: String,
    pub return_ty: Ty,
    pub params: Vec<Ty>,
    pub opcode: u8,
    pub is_interface: bool,
    pub is_varargs: bool,
    pub access_flags: u16,
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
    ClassCatalog::platform().resolve_static_field(owner, name)
}

pub fn resolve_instance_method(receiver: &Ty, name: &str, args: &[Ty]) -> Option<MethodRef> {
    ClassCatalog::platform().resolve_instance_method(receiver, name, args)
}

#[path = "../crates/javac-bytecode/src/class_gen.rs"]
pub mod class_gen;
#[path = "../crates/javac-bytecode/src/codegen.rs"]
pub mod codegen;
#[path = "../crates/javac-bytecode/src/error.rs"]
pub mod error;
#[path = "../crates/javac-bytecode/src/expr_gen.rs"]
pub mod expr_gen;
#[path = "../crates/javac-bytecode/src/local_var.rs"]
pub mod local_var;
#[path = "../crates/javac-bytecode/src/method_gen.rs"]
pub mod method_gen;
#[path = "../crates/javac-bytecode/src/stmt_gen.rs"]
pub mod stmt_gen;
#[path = "../crates/javac-bytecode/src/validation.rs"]
mod validation;

pub use error::BytecodeError;

#[path = "../crates/javac-compiler/src/classpath.rs"]
mod classpath;
#[path = "../crates/javac-compiler/src/compiler.rs"]
pub mod compiler;
#[path = "../crates/javac-compiler/src/config.rs"]
pub mod config;
#[path = "../crates/javac-compiler/src/pipeline.rs"]
pub mod pipeline;

pub use config::CompilerConfig;
pub use pipeline::compile;
