use crate::{FieldRef, MethodRef};
use javac_ty::Ty;
use rust_asm::opcodes;

pub fn resolve_static_field(_owner: &str, _name: &str) -> Option<FieldRef> {
    None
}

pub fn resolve_instance_method(receiver: &Ty, name: &str, args: &[Ty]) -> Option<MethodRef> {
    match (receiver.erasure(), name, args) {
        (Ty::Class(owner), "length", []) if owner.as_str() == "java/lang/String" => {
            Some(MethodRef {
                owner: "java/lang/String",
                name: "length",
                descriptor: "()I".to_string(),
                return_ty: Ty::Int,
                opcode: opcodes::INVOKEVIRTUAL,
                is_interface: false,
            })
        }
        (Ty::Class(owner), "charAt", [Ty::Int]) if owner.as_str() == "java/lang/String" => {
            Some(MethodRef {
                owner: "java/lang/String",
                name: "charAt",
                descriptor: "(I)C".to_string(),
                return_ty: Ty::Char,
                opcode: opcodes::INVOKEVIRTUAL,
                is_interface: false,
            })
        }
        _ => None,
    }
}
