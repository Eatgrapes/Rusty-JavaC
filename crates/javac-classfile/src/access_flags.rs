pub const ACC_PUBLIC: u16 = 0x0001;
pub const ACC_PRIVATE: u16 = 0x0002;
pub const ACC_PROTECTED: u16 = 0x0004;
pub const ACC_STATIC: u16 = 0x0008;
pub const ACC_FINAL: u16 = 0x0010;
pub const ACC_SUPER: u16 = 0x0020;
pub const ACC_SYNCHRONIZED: u16 = 0x0020;
pub const ACC_VOLATILE: u16 = 0x0040;
pub const ACC_BRIDGE: u16 = 0x0040;
pub const ACC_TRANSIENT: u16 = 0x0080;
pub const ACC_VARARGS: u16 = 0x0080;
pub const ACC_NATIVE: u16 = 0x0100;
pub const ACC_INTERFACE: u16 = 0x0200;
pub const ACC_ABSTRACT: u16 = 0x0400;
pub const ACC_STRICT: u16 = 0x0800;
pub const ACC_SYNTHETIC: u16 = 0x1000;
pub const ACC_ANNOTATION: u16 = 0x2000;
pub const ACC_ENUM: u16 = 0x4000;
pub const ACC_MODULE: u16 = 0x8000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessLevel {
    Public,
    Protected,
    Package,
    Private,
}

impl AccessLevel {
    pub fn flags(&self) -> u16 {
        match self {
            AccessLevel::Public => ACC_PUBLIC,
            AccessLevel::Protected => ACC_PROTECTED,
            AccessLevel::Package => 0,
            AccessLevel::Private => ACC_PRIVATE,
        }
    }

    pub fn from_flags(flags: u16) -> Self {
        if flags & ACC_PUBLIC != 0 {
            AccessLevel::Public
        } else if flags & ACC_PROTECTED != 0 {
            AccessLevel::Protected
        } else if flags & ACC_PRIVATE != 0 {
            AccessLevel::Private
        } else {
            AccessLevel::Package
        }
    }
}