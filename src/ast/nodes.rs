use super::{JavaSyntaxKind, JavaSyntaxNode, JavaSyntaxToken};

pub trait AstNode {
    fn can_cast(kind: JavaSyntaxKind) -> bool
    where
        Self: Sized;

    fn cast(syntax: JavaSyntaxNode) -> Option<Self>
    where
        Self: Sized;

    fn syntax(&self) -> &JavaSyntaxNode;
}

pub fn child<N: AstNode>(parent: &JavaSyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

pub fn children<N: AstNode>(parent: &JavaSyntaxNode) -> impl Iterator<Item = N> {
    parent.children().filter_map(N::cast)
}

pub fn token(parent: &JavaSyntaxNode, kind: JavaSyntaxKind) -> Option<JavaSyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|it| it.into_token())
        .find(|it| it.kind() == kind)
}

macro_rules! ast_node {
    ($ast:ident, $kind:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $ast(pub(crate) JavaSyntaxNode);

        impl AstNode for $ast {
            fn can_cast(kind: JavaSyntaxKind) -> bool {
                kind == JavaSyntaxKind::$kind
            }
            fn cast(syntax: JavaSyntaxNode) -> Option<Self> {
                if Self::can_cast(syntax.kind()) {
                    Some(Self(syntax))
                } else {
                    None
                }
            }
            fn syntax(&self) -> &JavaSyntaxNode {
                &self.0
            }
        }
    };
}

ast_node!(CompilationUnit, CompilationUnit);
impl CompilationUnit {
    pub fn package(&self) -> Option<PackageDecl> {
        child(&self.0)
    }
    pub fn imports(&self) -> impl Iterator<Item = ImportDecl> {
        children(&self.0)
    }
    pub fn type_decls(&self) -> impl Iterator<Item = TypeDecl> {
        children(&self.0)
    }
}

ast_node!(PackageDecl, PackageDecl);
impl PackageDecl {
    pub fn name(&self) -> Option<QualifiedName> {
        child(&self.0)
    }
}

ast_node!(ImportDecl, ImportDecl);
impl ImportDecl {
    pub fn name(&self) -> Option<QualifiedName> {
        child(&self.0)
    }
    pub fn is_static(&self) -> bool {
        token(&self.0, JavaSyntaxKind::StaticKw).is_some()
    }
    pub fn is_wildcard(&self) -> bool {
        token(&self.0, JavaSyntaxKind::Star).is_some()
    }
}

ast_node!(QualifiedName, QualifiedName);
ast_node!(Name, Name);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeDecl {
    Class(ClassDecl),
    Interface(InterfaceDecl),
    Enum(EnumDecl),
    Record(RecordDecl),
    Annotation(AnnotationDecl),
}

impl AstNode for TypeDecl {
    fn can_cast(kind: JavaSyntaxKind) -> bool {
        matches!(
            kind,
            JavaSyntaxKind::ClassDecl
                | JavaSyntaxKind::InterfaceDecl
                | JavaSyntaxKind::EnumDecl
                | JavaSyntaxKind::RecordDecl
                | JavaSyntaxKind::AnnotationDecl
        )
    }

    fn cast(syntax: JavaSyntaxNode) -> Option<Self> {
        match syntax.kind() {
            JavaSyntaxKind::ClassDecl => ClassDecl::cast(syntax).map(TypeDecl::Class),
            JavaSyntaxKind::InterfaceDecl => InterfaceDecl::cast(syntax).map(TypeDecl::Interface),
            JavaSyntaxKind::EnumDecl => EnumDecl::cast(syntax).map(TypeDecl::Enum),
            JavaSyntaxKind::RecordDecl => RecordDecl::cast(syntax).map(TypeDecl::Record),
            JavaSyntaxKind::AnnotationDecl => {
                AnnotationDecl::cast(syntax).map(TypeDecl::Annotation)
            }
            _ => None,
        }
    }

    fn syntax(&self) -> &JavaSyntaxNode {
        match self {
            TypeDecl::Class(it) => it.syntax(),
            TypeDecl::Interface(it) => it.syntax(),
            TypeDecl::Enum(it) => it.syntax(),
            TypeDecl::Record(it) => it.syntax(),
            TypeDecl::Annotation(it) => it.syntax(),
        }
    }
}

ast_node!(ClassDecl, ClassDecl);
impl ClassDecl {
    pub fn name(&self) -> Option<JavaSyntaxToken> {
        token(&self.0, JavaSyntaxKind::Ident)
    }
    pub fn body(&self) -> Option<ClassBody> {
        child(&self.0)
    }
}

ast_node!(InterfaceDecl, InterfaceDecl);
impl InterfaceDecl {
    pub fn name(&self) -> Option<JavaSyntaxToken> {
        token(&self.0, JavaSyntaxKind::Ident)
    }
    pub fn body(&self) -> Option<ClassBody> {
        child(&self.0)
    }
}

ast_node!(EnumDecl, EnumDecl);
impl EnumDecl {
    pub fn name(&self) -> Option<JavaSyntaxToken> {
        token(&self.0, JavaSyntaxKind::Ident)
    }
    pub fn body(&self) -> Option<EnumBody> {
        child(&self.0)
    }
}

ast_node!(RecordDecl, RecordDecl);
impl RecordDecl {
    pub fn name(&self) -> Option<JavaSyntaxToken> {
        token(&self.0, JavaSyntaxKind::Ident)
    }
    pub fn component_list(&self) -> Option<RecordComponentList> {
        child(&self.0)
    }
    pub fn body(&self) -> Option<ClassBody> {
        child(&self.0)
    }
}

ast_node!(AnnotationDecl, AnnotationDecl);
impl AnnotationDecl {
    pub fn name(&self) -> Option<JavaSyntaxToken> {
        token(&self.0, JavaSyntaxKind::Ident)
    }
    pub fn body(&self) -> Option<ClassBody> {
        child(&self.0)
    }
}

ast_node!(EnumBody, EnumBody);
ast_node!(RecordComponentList, RecordComponentList);

ast_node!(ClassBody, ClassBody);
impl ClassBody {
    pub fn members(&self) -> impl Iterator<Item = ClassMember> {
        children(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClassMember {
    Method(MethodDecl),
    Field(FieldDecl),
    Constructor(ConstructorDecl),
    Class(ClassDecl),
}

impl AstNode for ClassMember {
    fn can_cast(kind: JavaSyntaxKind) -> bool {
        matches!(
            kind,
            JavaSyntaxKind::MethodDecl
                | JavaSyntaxKind::FieldDecl
                | JavaSyntaxKind::ConstructorDecl
                | JavaSyntaxKind::ClassDecl
        )
    }

    fn cast(syntax: JavaSyntaxNode) -> Option<Self> {
        match syntax.kind() {
            JavaSyntaxKind::MethodDecl => MethodDecl::cast(syntax).map(ClassMember::Method),
            JavaSyntaxKind::FieldDecl => FieldDecl::cast(syntax).map(ClassMember::Field),
            JavaSyntaxKind::ConstructorDecl => {
                ConstructorDecl::cast(syntax).map(ClassMember::Constructor)
            }
            JavaSyntaxKind::ClassDecl => ClassDecl::cast(syntax).map(ClassMember::Class),
            _ => None,
        }
    }

    fn syntax(&self) -> &JavaSyntaxNode {
        match self {
            ClassMember::Method(it) => it.syntax(),
            ClassMember::Field(it) => it.syntax(),
            ClassMember::Constructor(it) => it.syntax(),
            ClassMember::Class(it) => it.syntax(),
        }
    }
}

ast_node!(MethodDecl, MethodDecl);
impl MethodDecl {
    pub fn name(&self) -> Option<JavaSyntaxToken> {
        token(&self.0, JavaSyntaxKind::Ident)
    }
    pub fn body(&self) -> Option<MethodBody> {
        child(&self.0)
    }
    pub fn return_type(&self) -> Option<Type> {
        child(&self.0)
    }
}

ast_node!(FieldDecl, FieldDecl);
impl FieldDecl {
    pub fn ty(&self) -> Option<Type> {
        child(&self.0)
    }
    // Declarators usually follow
}

ast_node!(ConstructorDecl, ConstructorDecl);

ast_node!(MethodBody, MethodBody);
impl MethodBody {
    pub fn block(&self) -> Option<Block> {
        child(&self.0)
    }
}

ast_node!(Block, Block);

ast_node!(Type, Type);

#[cfg(test)]
mod tests {
    use super::*;
    use rowan::GreenNodeBuilder;

    fn build_method_decl_node() -> JavaSyntaxNode {
        let mut builder = GreenNodeBuilder::new();

        builder.start_node(JavaSyntaxKind::MethodDecl.into());

        builder.token(JavaSyntaxKind::Ident.into(), "myMethod");

        builder.start_node(JavaSyntaxKind::MethodBody.into());
        builder.start_node(JavaSyntaxKind::Block.into());
        builder.finish_node(); // Block
        builder.finish_node(); // MethodBody

        builder.finish_node(); // MethodDecl

        let green = builder.finish();
        JavaSyntaxNode::new_root(green)
    }

    #[test]
    fn test_ast_node_cast_and_token() {
        let syntax = build_method_decl_node();

        // Test casting
        assert!(MethodDecl::can_cast(syntax.kind()));
        let method = MethodDecl::cast(syntax).expect("Should cast to MethodDecl");

        // Test token getter
        let name_token = method.name().expect("Should have a name token");
        assert_eq!(name_token.text(), "myMethod");
        assert_eq!(name_token.kind(), JavaSyntaxKind::Ident);
    }

    #[test]
    fn test_ast_node_child_access() {
        let syntax = build_method_decl_node();
        let method = MethodDecl::cast(syntax).unwrap();

        // Test child node getter
        let body = method.body().expect("Should have a body");
        assert!(MethodBody::can_cast(body.syntax().kind()));

        // Test nested child
        let block = body.block().expect("Body should have a block");
        assert!(Block::can_cast(block.syntax().kind()));
    }
}
