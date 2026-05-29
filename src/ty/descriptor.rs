use crate::ty::Ty;
use crate::ty::method_sig::MethodSig;
use ustr::Ustr;

pub fn field_descriptor(ty: &Ty) -> String {
    ty.erasure().descriptor()
}

pub fn method_descriptor(sig: &MethodSig) -> String {
    sig.descriptor()
}

pub fn class_descriptor(class_name: &str) -> String {
    format!("L{};", class_name.replace('.', "/"))
}

pub fn descriptor_to_ty(desc: &str) -> Option<Ty> {
    let chars = desc.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return None;
    }
    let (ty, _) = parse_type(&chars, 0)?;
    Some(ty)
}

pub fn method_descriptor_to_sig(name: &str, desc: &str) -> Option<MethodSig> {
    let chars = desc.chars().collect::<Vec<_>>();
    if chars.first().copied()? != '(' {
        return None;
    }

    let mut params = Vec::new();
    let mut pos = 1;
    while chars.get(pos).copied()? != ')' {
        let (param, next) = parse_type(&chars, pos)?;
        params.push(param);
        pos = next;
    }

    let (return_type, end) = parse_type(&chars, pos + 1)?;
    (end == chars.len()).then(|| MethodSig::new(Ustr::from(name), params, return_type))
}

fn parse_type(chars: &[char], pos: usize) -> Option<(Ty, usize)> {
    if pos >= chars.len() {
        return None;
    }
    match chars[pos] {
        'V' => Some((Ty::Void, pos + 1)),
        'Z' => Some((Ty::Boolean, pos + 1)),
        'B' => Some((Ty::Byte, pos + 1)),
        'C' => Some((Ty::Char, pos + 1)),
        'S' => Some((Ty::Short, pos + 1)),
        'I' => Some((Ty::Int, pos + 1)),
        'J' => Some((Ty::Long, pos + 1)),
        'F' => Some((Ty::Float, pos + 1)),
        'D' => Some((Ty::Double, pos + 1)),
        'L' => {
            let end = chars.iter().skip(pos + 1).position(|&c| c == ';')?;
            let name: String = chars[pos + 1..pos + 1 + end].iter().collect();
            Some((Ty::Class(ustr::Ustr::from(&name)), pos + 1 + end + 1))
        }
        '[' => {
            let (elem, next) = parse_type(chars, pos + 1)?;
            Some((Ty::Array(Box::new(elem)), next))
        }
        _ => None,
    }
}
