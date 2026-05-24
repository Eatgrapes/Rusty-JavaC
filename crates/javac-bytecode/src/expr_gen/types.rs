use crate::codegen::CodegenCtx;
use javac_hir::hir::*;
use javac_hir::infer::{self, TypeEnvironment};
use javac_ty::Ty;

pub(crate) fn expr_ty(ctx: &CodegenCtx, body: &Body, expr_id: ExprId) -> Ty {
    infer::expr_ty(ctx, body, expr_id)
}

impl TypeEnvironment for CodegenCtx<'_> {
    fn local_ty(&self, name: ustr::Ustr) -> Option<Ty> {
        CodegenCtx::local_ty(self, name)
    }

    fn field_ty(&self, name: ustr::Ustr) -> Option<Ty> {
        CodegenCtx::field_ty(self, name)
    }

    fn resolve_static_field(&self, owner: &str, name: &str) -> Option<Ty> {
        self.catalog
            .resolve_static_field(owner, name)
            .map(|field| field.ty)
    }

    fn resolve_instance_method(&self, receiver: &Ty, name: &str, args: &[Ty]) -> Option<Ty> {
        self.catalog
            .resolve_instance_method(receiver, name, args)
            .map(|method| method.return_ty)
    }

    fn resolve_current_method(&self, name: ustr::Ustr, _args: &[Ty]) -> Option<Ty> {
        self.method_sig(name).map(|sig| sig.return_type)
    }

    fn this_ty(&self) -> Ty {
        Ty::Class(self.class_name)
    }

    fn super_ty(&self) -> Ty {
        Ty::Class(self.super_name)
    }
}
