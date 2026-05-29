pub use rust_asm::constant_pool;

pub struct ConstantPool;

impl ConstantPool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConstantPool {
    fn default() -> Self {
        Self::new()
    }
}
