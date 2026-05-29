use std::fmt;
use ustr::Ustr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    Void,
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
    Class(Ustr),
    Array(Box<Ty>),
    TypeVar(Ustr),
    Wildcard(Option<Box<Ty>>),
    Intersection(Vec<Ty>),
}

impl Ty {
    pub fn object() -> Self {
        Ty::Class(Ustr::from("java/lang/Object"))
    }

    pub fn string() -> Self {
        Ty::Class(Ustr::from("java/lang/String"))
    }

    pub fn class(internal_name: impl AsRef<str>) -> Self {
        Ty::Class(Ustr::from(internal_name.as_ref()))
    }

    pub fn is_string(&self) -> bool {
        matches!(self.erasure(), Ty::Class(name) if name.as_str() == "java/lang/String")
    }

    pub fn descriptor(&self) -> String {
        match self {
            Ty::Void => "V".to_string(),
            Ty::Boolean => "Z".to_string(),
            Ty::Byte => "B".to_string(),
            Ty::Char => "C".to_string(),
            Ty::Short => "S".to_string(),
            Ty::Int => "I".to_string(),
            Ty::Long => "J".to_string(),
            Ty::Float => "F".to_string(),
            Ty::Double => "D".to_string(),
            Ty::Class(name) => format!("L{};", name.as_str().replace('.', "/")),
            Ty::Array(elem) => format!("[{}", elem.descriptor()),
            Ty::TypeVar(_) => "Ljava/lang/Object;".to_string(),
            Ty::Wildcard(_) => "Ljava/lang/Object;".to_string(),
            Ty::Intersection(_) => "Ljava/lang/Object;".to_string(),
        }
    }

    pub fn erasure(&self) -> Ty {
        match self {
            Ty::TypeVar(_) => Ty::object(),
            Ty::Wildcard(Some(bound)) => bound.erasure(),
            Ty::Wildcard(None) => Ty::object(),
            Ty::Intersection(types) => types
                .first()
                .map(|t| t.erasure())
                .unwrap_or_else(Ty::object),
            other => other.clone(),
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            Ty::Boolean
                | Ty::Byte
                | Ty::Char
                | Ty::Short
                | Ty::Int
                | Ty::Long
                | Ty::Float
                | Ty::Double
        )
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Ty::Byte | Ty::Short | Ty::Int | Ty::Long | Ty::Float | Ty::Double | Ty::Char
        )
    }

    pub fn is_integral(&self) -> bool {
        matches!(self, Ty::Byte | Ty::Short | Ty::Int | Ty::Long | Ty::Char)
    }

    pub fn is_floating(&self) -> bool {
        matches!(self, Ty::Float | Ty::Double)
    }

    pub fn size(&self) -> usize {
        match self {
            Ty::Long | Ty::Double => 2,
            Ty::Void => 0,
            _ => 1,
        }
    }

    pub fn internal_name(&self) -> String {
        match self {
            Ty::Class(name) => name.as_str().to_string(),
            Ty::Array(elem) => elem.descriptor(),
            _ => self.erasure().internal_name(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeParam {
    pub name: Ustr,
    pub bounds: Vec<Ty>,
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Void => write!(f, "void"),
            Ty::Boolean => write!(f, "boolean"),
            Ty::Byte => write!(f, "byte"),
            Ty::Char => write!(f, "char"),
            Ty::Short => write!(f, "short"),
            Ty::Int => write!(f, "int"),
            Ty::Long => write!(f, "long"),
            Ty::Float => write!(f, "float"),
            Ty::Double => write!(f, "double"),
            Ty::Class(name) => write!(f, "{}", name.as_str().replace('/', ".")),
            Ty::Array(elem) => write!(f, "{}[]", elem),
            Ty::TypeVar(name) => write!(f, "{}", name.as_str()),
            Ty::Wildcard(Some(bound)) => write!(f, "? extends {}", bound),
            Ty::Wildcard(None) => write!(f, "?"),
            Ty::Intersection(types) => {
                let parts: Vec<String> = types.iter().map(|t| t.to_string()).collect();
                write!(f, "{}", parts.join(" & "))
            }
        }
    }
}
