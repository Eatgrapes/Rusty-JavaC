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

pub fn lower(node: &JavaSyntaxNode) -> LowerResult<CompilationUnit> {
    unit::lower_compilation_unit(node)
}
