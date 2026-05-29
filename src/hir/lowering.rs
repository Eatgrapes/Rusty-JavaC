#[path = "lowering/anonymous.rs"]
mod anonymous;
#[path = "lowering/error.rs"]
mod error;
#[path = "lowering/expr.rs"]
mod expr;
#[path = "lowering/literal.rs"]
mod literal;
#[path = "lowering/member.rs"]
mod member;
#[path = "lowering/modifiers.rs"]
mod modifiers;
#[path = "lowering/signature.rs"]
mod signature;
#[path = "lowering/stmt.rs"]
mod stmt;
#[path = "lowering/syntax.rs"]
mod syntax;
#[path = "lowering/types.rs"]
mod types;
#[path = "lowering/unit.rs"]
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
