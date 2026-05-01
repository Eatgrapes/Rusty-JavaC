use crate::hir::*;

pub fn collect_items(unit: &CompilationUnit) -> Vec<Item> {
    let mut items = Vec::new();
    for td in &unit.type_decls {
        collect_type_items(td, &mut items);
    }
    items
}

fn collect_type_items(td: &TypeDecl, items: &mut Vec<Item>) {
    items.push(Item::Type(td.clone()));
    for f in &td.fields {
        items.push(Item::Field(f.clone()));
    }
    for m in &td.methods {
        items.push(Item::Method(m.clone()));
    }
    for inner in &td.inner_types {
        collect_type_items(inner, items);
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Type(TypeDecl),
    Field(FieldDecl),
    Method(MethodDecl),
}