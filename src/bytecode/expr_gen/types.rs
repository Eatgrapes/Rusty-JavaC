use crate::bytecode::codegen::CodegenCtx;
use crate::hir::infer::{self, TypeEnvironment};
use crate::hir::*;
use crate::ty::Ty;

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
            .or_else(|| {
                if owner != self.class_name.as_str() {
                    return None;
                }
                self.fields
                    .get(&ustr::Ustr::from(name))
                    .filter(|field| field.access_flags & crate::classfile::ACC_STATIC != 0)
                    .map(|field| field.ty.clone())
            })
    }

    fn resolve_instance_method(&self, receiver: &Ty, name: &str, args: &[Ty]) -> Option<Ty> {
        if matches!(receiver.erasure(), Ty::Class(owner) if owner == self.class_name) {
            if let Some(sig) = self.method_sig(ustr::Ustr::from(name))
                && sig.access_flags & crate::classfile::ACC_STATIC == 0
            {
                return Some(sig.return_type);
            }
            return self
                .catalog
                .resolve_instance_method(&Ty::Class(self.super_name), name, args)
                .map(|method| method.return_ty);
        }
        self.catalog
            .resolve_instance_method(receiver, name, args)
            .map(|method| method.return_ty)
    }

    fn resolve_static_method(&self, owner: &str, name: &str, args: &[Ty]) -> Option<Ty> {
        self.catalog
            .resolve_static_method(owner, name, args)
            .map(|method| method.return_ty)
            .or_else(|| {
                (owner == self.class_name.as_str()).then(|| {
                    self.method_sig(ustr::Ustr::from(name))
                        .filter(|sig| sig.access_flags & crate::classfile::ACC_STATIC != 0)
                        .map(|sig| sig.return_type)
                })?
            })
    }

    fn resolve_current_method(&self, name: ustr::Ustr, _args: &[Ty]) -> Option<Ty> {
        self.method_sig(name)
            .map(|sig| sig.return_type)
            .or_else(|| {
                self.catalog
                    .resolve_instance_method(&Ty::Class(self.super_name), name.as_str(), _args)
                    .map(|method| method.return_ty)
                    .or_else(|| {
                        self.enclosing_static_owner.and_then(|owner| {
                            self.catalog
                                .resolve_static_method(owner.as_str(), name.as_str(), _args)
                                .map(|method| method.return_ty)
                        })
                    })
            })
    }

    fn this_ty(&self) -> Ty {
        Ty::Class(self.class_name)
    }

    fn super_ty(&self) -> Ty {
        Ty::Class(self.super_name)
    }
}
