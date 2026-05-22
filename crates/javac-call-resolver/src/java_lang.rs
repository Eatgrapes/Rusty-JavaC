use crate::{FieldRef, MethodRef};
use javac_ty::Ty;
use rust_asm::opcodes;

const STRING_CLASS: &str = "java/lang/String";

pub fn class_name(simple_name: &str) -> Option<&'static str> {
    match simple_name {
        "String" => Some(STRING_CLASS),
        _ => None,
    }
}

pub fn resolve_static_field(_owner: &str, _name: &str) -> Option<FieldRef> {
    None
}

pub fn resolve_instance_method(receiver: &Ty, name: &str, args: &[Ty]) -> Option<MethodRef> {
    match (receiver.erasure(), name, args) {
        (Ty::Class(owner), "length", []) if owner.as_str() == STRING_CLASS => Some(MethodRef {
            owner: STRING_CLASS,
            name: "length",
            descriptor: "()I".to_string(),
            return_ty: Ty::Int,
            opcode: opcodes::INVOKEVIRTUAL,
            is_interface: false,
        }),
        (Ty::Class(owner), "charAt", [Ty::Int]) if owner.as_str() == STRING_CLASS => {
            Some(MethodRef {
                owner: STRING_CLASS,
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
