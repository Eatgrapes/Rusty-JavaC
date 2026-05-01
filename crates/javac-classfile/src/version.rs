pub const V21: u16 = 65;
pub const V17: u16 = 61;
pub const V11: u16 = 55;
pub const V8: u16 = 52;

pub fn version_for_java(major: u32) -> u16 {
    match major {
        8 => V8,
        11 => V11,
        17 => V17,
        21 => V21,
        n if n >= 45 => n as u16,
        _ => V21,
    }
}