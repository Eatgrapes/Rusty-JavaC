mod annotation;
mod anonymous;
mod enum_decl;
mod error;
mod expr;
mod literal;
mod member;
mod modifiers;
mod record;
mod signature;
mod stmt;
mod syntax;
mod types;
mod unit;

use crate::ast::JavaSyntaxNode;
use crate::call_resolver::ClassCatalog;
use crate::hir::CompilationUnit;
pub use error::{LowerError, LowerResult};

pub fn lower(node: &JavaSyntaxNode) -> LowerResult<CompilationUnit> {
    let catalog = ClassCatalog::platform();
    lower_with_catalog(node, &catalog)
}

pub fn lower_with_catalog(
    node: &JavaSyntaxNode,
    catalog: &ClassCatalog,
) -> LowerResult<CompilationUnit> {
    unit::lower_compilation_unit(node, catalog)
}
