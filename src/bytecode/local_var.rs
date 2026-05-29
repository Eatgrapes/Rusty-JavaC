use crate::ty::Ty;

pub fn store_opcode(ty: &Ty) -> u8 {
    use rust_asm::opcodes;
    match ty {
        Ty::Int | Ty::Boolean | Ty::Byte | Ty::Char | Ty::Short => opcodes::ISTORE,
        Ty::Long => opcodes::LSTORE,
        Ty::Float => opcodes::FSTORE,
        Ty::Double => opcodes::DSTORE,
        _ => opcodes::ASTORE,
    }
}

pub fn load_opcode(ty: &Ty) -> u8 {
    use rust_asm::opcodes;
    match ty {
        Ty::Int | Ty::Boolean | Ty::Byte | Ty::Char | Ty::Short => opcodes::ILOAD,
        Ty::Long => opcodes::LLOAD,
        Ty::Float => opcodes::FLOAD,
        Ty::Double => opcodes::DLOAD,
        _ => opcodes::ALOAD,
    }
}

pub fn return_opcode(ty: &Ty) -> u8 {
    use rust_asm::opcodes;
    match ty {
        Ty::Int | Ty::Boolean | Ty::Byte | Ty::Char | Ty::Short => opcodes::IRETURN,
        Ty::Long => opcodes::LRETURN,
        Ty::Float => opcodes::FRETURN,
        Ty::Double => opcodes::DRETURN,
        Ty::Void => opcodes::RETURN,
        _ => opcodes::ARETURN,
    }
}
