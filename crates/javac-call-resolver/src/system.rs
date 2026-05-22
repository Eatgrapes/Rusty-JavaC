use crate::{FieldRef, MethodRef};
use javac_ty::Ty;
use rust_asm::opcodes;
use ustr::Ustr;

const SYSTEM_CLASS: &str = "java/lang/System";
const PRINT_STREAM_CLASS: &str = "java/io/PrintStream";

pub fn class_name(simple_name: &str) -> Option<&'static str> {
    match simple_name {
        "System" => Some(SYSTEM_CLASS),
        _ => None,
    }
}

pub fn resolve_static_field(owner: &str, name: &str) -> Option<FieldRef> {
    match (owner, name) {
        (SYSTEM_CLASS, "out") => Some(FieldRef {
            owner: SYSTEM_CLASS,
            name: "out",
            descriptor: "Ljava/io/PrintStream;",
            ty: Ty::Class(Ustr::from(PRINT_STREAM_CLASS)),
        }),
        _ => None,
    }
}

pub fn resolve_instance_method(receiver: &Ty, name: &str, args: &[Ty]) -> Option<MethodRef> {
    match (receiver.erasure(), name) {
        (Ty::Class(owner), "println") if owner.as_str() == PRINT_STREAM_CLASS => Some(MethodRef {
            owner: PRINT_STREAM_CLASS,
            name: "println",
            descriptor: void_method_descriptor(args),
            return_ty: Ty::Void,
            opcode: opcodes::INVOKEVIRTUAL,
            is_interface: false,
        }),
        _ => None,
    }
}

fn void_method_descriptor(args: &[Ty]) -> String {
    let mut descriptor = String::from("(");
    for arg in args {
        descriptor.push_str(&arg.erasure().descriptor());
    }
    descriptor.push_str(")V");
    descriptor
}
