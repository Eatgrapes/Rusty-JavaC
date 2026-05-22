use javac_ast::{JavaSyntaxKind, JavaSyntaxNode};

pub(super) const ACC_PUBLIC: u16 = 0x0001;
const ACC_PRIVATE: u16 = 0x0002;
const ACC_PROTECTED: u16 = 0x0004;
const ACC_STATIC: u16 = 0x0008;
const ACC_FINAL: u16 = 0x0010;
const ACC_SYNCHRONIZED: u16 = 0x0020;
pub(super) const ACC_NATIVE: u16 = 0x0100;
pub(super) const ACC_ABSTRACT: u16 = 0x0400;

pub(super) fn access_flags(node: &JavaSyntaxNode) -> u16 {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .fold(0, |flags, token| match token.kind() {
            JavaSyntaxKind::PublicKw => flags | ACC_PUBLIC,
            JavaSyntaxKind::PrivateKw => flags | ACC_PRIVATE,
            JavaSyntaxKind::ProtectedKw => flags | ACC_PROTECTED,
            JavaSyntaxKind::StaticKw => flags | ACC_STATIC,
            JavaSyntaxKind::FinalKw => flags | ACC_FINAL,
            JavaSyntaxKind::SynchronizedKw => flags | ACC_SYNCHRONIZED,
            JavaSyntaxKind::NativeKw => flags | ACC_NATIVE,
            JavaSyntaxKind::AbstractKw => flags | ACC_ABSTRACT,
            _ => flags,
        })
}

pub(super) fn has_code(access_flags: u16) -> bool {
    access_flags & (ACC_ABSTRACT | ACC_NATIVE) == 0
}
