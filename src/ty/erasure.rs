use crate::ty::Ty;

pub fn erasure(ty: &Ty) -> Ty {
    ty.erasure()
}

pub fn erasure_descriptor(ty: &Ty) -> String {
    ty.erasure().descriptor()
}
