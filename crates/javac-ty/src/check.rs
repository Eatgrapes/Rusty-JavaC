use crate::ty::Ty;
use ustr::Ustr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeError {
    IncompatibleTypes { expected: Ty, found: Ty },
    NotAccessible { name: Ustr },
    NotAssignable { from: Ty, to: Ty },
    Ambiguous { name: Ustr },
    NotFound { name: Ustr },
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
    matches!(
        (from, to),
        (
            Ty::Byte,
            Ty::Short | Ty::Int | Ty::Long | Ty::Float | Ty::Double
        ) | (Ty::Short, Ty::Int | Ty::Long | Ty::Float | Ty::Double)
            | (Ty::Char, Ty::Int | Ty::Long | Ty::Float | Ty::Double)
            | (Ty::Int, Ty::Long | Ty::Float | Ty::Double)
            | (Ty::Long, Ty::Float | Ty::Double)
            | (Ty::Float, Ty::Double)
    )
}

pub fn is_narrowing_primitive(from: &Ty, to: &Ty) -> bool {
    matches!(
        (from, to),
        (Ty::Short, Ty::Byte | Ty::Char)
            | (Ty::Char, Ty::Byte | Ty::Short)
            | (Ty::Int, Ty::Byte | Ty::Short | Ty::Char)
            | (Ty::Long, Ty::Byte | Ty::Short | Ty::Char | Ty::Int)
            | (
                Ty::Float,
                Ty::Byte | Ty::Short | Ty::Char | Ty::Int | Ty::Long
            )
            | (
                Ty::Double,
                Ty::Byte | Ty::Short | Ty::Char | Ty::Int | Ty::Long | Ty::Float
            )
    )
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
        Ty::Boolean => Some(Ty::Class(Ustr::from("java/lang/Boolean"))),
        Ty::Byte => Some(Ty::Class(Ustr::from("java/lang/Byte"))),
        Ty::Char => Some(Ty::Class(Ustr::from("java/lang/Character"))),
        Ty::Short => Some(Ty::Class(Ustr::from("java/lang/Short"))),
        Ty::Int => Some(Ty::Class(Ustr::from("java/lang/Integer"))),
        Ty::Long => Some(Ty::Class(Ustr::from("java/lang/Long"))),
        Ty::Float => Some(Ty::Class(Ustr::from("java/lang/Float"))),
        Ty::Double => Some(Ty::Class(Ustr::from("java/lang/Double"))),
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
