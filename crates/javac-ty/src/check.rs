use crate::ty::Ty;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeError {
    IncompatibleTypes { expected: Ty, found: Ty },
    NotAccessible { name: String },
    NotAssignable { from: Ty, to: Ty },
    Ambiguous { name: String },
    NotFound { name: String },
}

pub fn is_assignable(from: &Ty, to: &Ty) -> bool {
    if from == to {
        return true;
    }
    let from_erased = from.erasure();
    let to_erased = to.erasure();
    if from_erased == to_erased {
        return true;
    }
    if is_widening_primitive(&from_erased, &to_erased) {
        return true;
    }
    false
}

fn is_widening_primitive(from: &Ty, to: &Ty) -> bool {
    match (from, to) {
        (Ty::Byte, Ty::Short | Ty::Int | Ty::Long | Ty::Float | Ty::Double) => true,
        (Ty::Short, Ty::Int | Ty::Long | Ty::Float | Ty::Double) => true,
        (Ty::Char, Ty::Int | Ty::Long | Ty::Float | Ty::Double) => true,
        (Ty::Int, Ty::Long | Ty::Float | Ty::Double) => true,
        (Ty::Long, Ty::Float | Ty::Double) => true,
        (Ty::Float, Ty::Double) => true,
        _ => false,
    }
}

pub fn is_narrowing_primitive(from: &Ty, to: &Ty) -> bool {
    match (from, to) {
        (Ty::Short, Ty::Byte | Ty::Char) => true,
        (Ty::Char, Ty::Byte | Ty::Short) => true,
        (Ty::Int, Ty::Byte | Ty::Short | Ty::Char) => true,
        (Ty::Long, Ty::Byte | Ty::Short | Ty::Char | Ty::Int) => true,
        (Ty::Float, Ty::Byte | Ty::Short | Ty::Char | Ty::Int | Ty::Long) => true,
        (Ty::Double, Ty::Byte | Ty::Short | Ty::Char | Ty::Int | Ty::Long | Ty::Float) => true,
        _ => false,
    }
}

pub fn unboxing_type(ty: &Ty) -> Option<Ty> {
    match ty {
        Ty::Class(name) => match name.as_str() {
            "java/lang/Boolean" => Some(Ty::Boolean),
            "java/lang/Byte" => Some(Ty::Byte),
            "java/lang/Character" => Some(Ty::Char),
            "java/lang/Short" => Some(Ty::Short),
            "java/lang/Integer" => Some(Ty::Int),
            "java/lang/Long" => Some(Ty::Long),
            "java/lang/Float" => Some(Ty::Float),
            "java/lang/Double" => Some(Ty::Double),
            _ => None,
        },
        _ => None,
    }
}

pub fn boxing_type(ty: &Ty) -> Option<Ty> {
    match ty {
        Ty::Boolean => Some(Ty::Class("java/lang/Boolean".to_string())),
        Ty::Byte => Some(Ty::Class("java/lang/Byte".to_string())),
        Ty::Char => Some(Ty::Class("java/lang/Character".to_string())),
        Ty::Short => Some(Ty::Class("java/lang/Short".to_string())),
        Ty::Int => Some(Ty::Class("java/lang/Integer".to_string())),
        Ty::Long => Some(Ty::Class("java/lang/Long".to_string())),
        Ty::Float => Some(Ty::Class("java/lang/Float".to_string())),
        Ty::Double => Some(Ty::Class("java/lang/Double".to_string())),
        _ => None,
    }
}

pub fn numeric_promotion(left: &Ty, right: &Ty) -> Option<Ty> {
    if !left.is_numeric() || !right.is_numeric() {
        return None;
    }
    if left == &Ty::Double || right == &Ty::Double {
        return Some(Ty::Double);
    }
    if left == &Ty::Float || right == &Ty::Float {
        return Some(Ty::Float);
    }
    if left == &Ty::Long || right == &Ty::Long {
        return Some(Ty::Long);
    }
    Some(Ty::Int)
}