use crate::{FieldRef, MethodRef};
use javac_ty::Ty;
use rust_asm::opcodes;
use ustr::Ustr;

pub fn class_name(simple_name: &str) -> Option<&'static str> {
    match simple_name {
        "System" => Some("java/lang/System"),
        _ => None,
    }
}

pub fn resolve_static_field(owner: &str, name: &str) -> Option<FieldRef> {
    match (owner, name) {
        ("java/lang/System", "out") => Some(FieldRef {
            owner: "java/lang/System",
            name: "out",
            descriptor: "Ljava/io/PrintStream;",
            ty: Ty::Class(Ustr::from("java/io/PrintStream")),
            is_static: true,
        }),
        _ => None,
    }
}

pub fn resolve_instance_method(receiver: &Ty, name: &str, args: &[Ty]) -> Option<MethodRef> {
    match (receiver.erasure(), name) {
        (Ty::Class(owner), "println") if owner.as_str() == "java/io/PrintStream" => {
            Some(MethodRef {
                owner: "java/io/PrintStream",
                name: "println",
                descriptor: print_stream_descriptor(args),
                return_ty: Ty::Void,
                opcode: opcodes::INVOKEVIRTUAL,
                is_interface: false,
            })
        }
        _ => None,
    }
}

fn print_stream_descriptor(args: &[Ty]) -> String {
    let mut descriptor = String::from("(");
    for arg in args {
        descriptor.push_str(&arg.erasure().descriptor());
    }
    descriptor.push_str(")V");
    descriptor
}
