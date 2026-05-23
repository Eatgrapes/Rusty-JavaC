use crate::parser::type_decl;
use crate::parser::{JavaSyntaxKind, Parser};

pub(crate) fn compilation_unit(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    if p.at(PackageKw) {
        package_decl(p);
    }
    while p.at(ImportKw) {
        import_decl(p);
    }
    while p.at_any(&[
        ClassKw,
        InterfaceKw,
        EnumKw,
        RecordKw,
        At,
        PublicKw,
        ProtectedKw,
        PrivateKw,
        AbstractKw,
        FinalKw,
        StaticKw,
        StrictfpKw,
        SealedKw,
        NonSealedKw,
    ]) {
        type_decl::type_decl(p);
    }
    m.complete(p, CompilationUnit);
}

pub(crate) fn package_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(PackageKw);
    qualified_name(p);
    p.expect(Semi);
    m.complete(p, PackageDecl);
}

pub(crate) fn import_decl(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(ImportKw);
    p.eat(StaticKw);
    qualified_name(p);
    if p.at(Dot) && p.look(1) == Star {
        p.bump();
        p.bump();
    }
    p.expect(Semi);
    m.complete(p, ImportDecl);
}

pub(crate) fn modifier_list(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    let mods = [
        PublicKw,
        ProtectedKw,
        PrivateKw,
        AbstractKw,
        FinalKw,
        StaticKw,
        StrictfpKw,
        SealedKw,
        NonSealedKw,
        NativeKw,
        SynchronizedKw,
        TransientKw,
        VolatileKw,
        DefaultKw,
    ];
    loop {
        if p.at_any(&mods) {
            p.bump();
        } else if p.at(At) {
            annotation(p);
        } else {
            break;
        }
    }
    m.complete(p, ModifierList);
}

pub(crate) fn qualified_name(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(Ident);
    while p.at(Dot) && p.look(1) == Ident {
        p.bump();
        p.expect(Ident);
    }
    m.complete(p, QualifiedName);
}

pub(crate) fn annotation(p: &mut Parser) {
    use JavaSyntaxKind::*;
    let m = p.start();
    p.expect(At);
    qualified_name(p);
    if p.at(LParen) {
        crate::parser::expr::argument_list(p);
    }
    m.complete(p, Annotation);
}
