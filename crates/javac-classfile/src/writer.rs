use rust_asm::class_writer::{ClassWriter, COMPUTE_FRAMES, MethodVisitor, FieldVisitor};
use crate::version;

pub struct ClassFileWriter {
    cw: ClassWriter,
}

impl ClassFileWriter {
    pub fn new() -> Self {
        Self {
            cw: ClassWriter::new(COMPUTE_FRAMES),
        }
    }

    pub fn visit(
        &mut self,
        java_version: u32,
        access_flags: u16,
        name: &str,
        super_name: Option<&str>,
        interfaces: &[&str],
    ) {
        let major = version::version_for_java(java_version);
        self.cw.visit(major, 0, access_flags, name, super_name, interfaces);
    }

    pub fn visit_method(
        &mut self,
        access_flags: u16,
        name: &str,
        descriptor: &str,
    ) -> MethodWriter {
        let mv = self.cw.visit_method(access_flags, name, descriptor);
        MethodWriter { inner: mv }
    }

    pub fn visit_field(
        &mut self,
        access_flags: u16,
        name: &str,
        descriptor: &str,
    ) -> FieldWriter {
        let fv = self.cw.visit_field(access_flags, name, descriptor);
        FieldWriter { inner: fv }
    }

    pub fn to_bytes(self) -> Result<Vec<u8>, String> {
        self.cw.to_bytes().map_err(|e| format!("{:?}", e))
    }
}

pub struct MethodWriter {
    inner: MethodVisitor,
}

impl MethodWriter {
    pub fn visit_code(&mut self) {
        self.inner.visit_code();
    }

    pub fn visit_insn(&mut self, opcode: u8) {
        self.inner.visit_insn(opcode);
    }

    pub fn visit_var_insn(&mut self, opcode: u8, var_index: u16) {
        self.inner.visit_var_insn(opcode, var_index);
    }

    pub fn visit_type_insn(&mut self, opcode: u8, type_name: &str) {
        self.inner.visit_type_insn(opcode, type_name);
    }

    pub fn visit_field_insn(&mut self, opcode: u8, owner: &str, name: &str, descriptor: &str) {
        self.inner.visit_field_insn(opcode, owner, name, descriptor);
    }

    pub fn visit_method_insn(
        &mut self,
        opcode: u8,
        owner: &str,
        name: &str,
        descriptor: &str,
        is_interface: bool,
    ) {
        self.inner.visit_method_insn(opcode, owner, name, descriptor, is_interface);
    }

    pub fn visit_ldc_insn_int(&mut self, value: i32) {
        self.inner.visit_ldc_insn(rust_asm::insn::LdcInsnNode::int(value));
    }

    pub fn visit_ldc_insn_float(&mut self, value: f32) {
        self.inner.visit_ldc_insn(rust_asm::insn::LdcInsnNode::float(value));
    }

    pub fn visit_ldc_insn_long(&mut self, value: i64) {
        self.inner.visit_ldc_insn(rust_asm::insn::LdcInsnNode::long(value));
    }

    pub fn visit_ldc_insn_double(&mut self, value: f64) {
        self.inner.visit_ldc_insn(rust_asm::insn::LdcInsnNode::double(value));
    }

    pub fn visit_ldc_insn_string(&mut self, value: &str) {
        self.inner.visit_ldc_insn(rust_asm::insn::LdcInsnNode::string(value));
    }

    pub fn visit_ldc_insn_type(&mut self, type_name: &str) {
        self.inner.visit_ldc_insn(rust_asm::insn::LdcInsnNode::typed(
            rust_asm::types::Type::get_object_type(type_name),
        ));
    }

    pub fn visit_iinc_insn(&mut self, var_index: u16, increment: i16) {
        self.inner.visit_iinc_insn(var_index, increment);
    }

    pub fn visit_maxs(&mut self, max_stack: u16, max_locals: u16) {
        self.inner.visit_maxs(max_stack, max_locals);
    }

    pub fn visit_end(self, cw: &mut ClassFileWriter) {
        self.inner.visit_end(&mut cw.cw);
    }
}

pub struct FieldWriter {
    inner: FieldVisitor,
}

impl FieldWriter {
    pub fn visit_end(self, cw: &mut ClassFileWriter) {
        self.inner.visit_end(&mut cw.cw);
    }
}