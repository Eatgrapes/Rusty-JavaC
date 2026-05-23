use crate::MethodRef;
use javac_ty::Ty;
use rust_asm::opcodes;

const FILE_INPUT_STREAM: &str = "java/io/FileInputStream";

pub(super) fn resolve_instance_method(receiver: &Ty, name: &str, args: &[Ty]) -> Option<MethodRef> {
    match (receiver.erasure(), name, args) {
        (Ty::Class(owner), "read", []) if owner.as_str() == FILE_INPUT_STREAM => Some(MethodRef {
            owner: FILE_INPUT_STREAM,
            name: "read",
            descriptor: "()I".to_string(),
            return_ty: Ty::Int,
            opcode: opcodes::INVOKEVIRTUAL,
            is_interface: false,
        }),
        (Ty::Class(owner), "close", []) if owner.as_str() == FILE_INPUT_STREAM => Some(MethodRef {
            owner: FILE_INPUT_STREAM,
            name: "close",
            descriptor: "()V".to_string(),
            return_ty: Ty::Void,
            opcode: opcodes::INVOKEVIRTUAL,
            is_interface: false,
        }),
        _ => None,
    }
}
