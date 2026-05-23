mod error;
mod expr;
mod literal;
mod member;
mod modifiers;
mod signature;
mod stmt;
mod syntax;
mod types;
mod unit;

use crate::hir::CompilationUnit;
pub use error::{LowerError, LowerResult};
use javac_ast::JavaSyntaxNode;
use javac_call_resolver::ClassCatalog;

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
