use rust_asm::class_reader::{AttributeInfo, LocalVariable, read_class_file};
use rust_asm::class_writer::{
    COMPUTE_FRAMES, COMPUTE_MAXS, ClassWriter as AsmClassWriter, FieldVisitor, MethodVisitor,
};
use rust_asm::constant_pool::{ConstantPoolBuilder, CpInfo};
pub use rust_asm::insn::Label;
use rust_asm::insn::LabelNode;
pub use rust_asm::insn::{BootstrapArgument, Handle};
use std::collections::HashMap;

pub struct ClassFileWriter {
    cw: AsmClassWriter,
    class_signature: Option<String>,
    nest_host: Option<String>,
    nest_members: Vec<String>,
    method_metadata: Vec<MethodMetadata>,
    field_metadata: Vec<FieldMetadata>,
}

impl ClassFileWriter {
    pub fn new() -> Self {
        Self {
            cw: AsmClassWriter::new(COMPUTE_FRAMES | COMPUTE_MAXS),
            class_signature: None,
            nest_host: None,
            nest_members: Vec::new(),
            method_metadata: Vec::new(),
            field_metadata: Vec::new(),
        }
    }

    pub fn visit(
        &mut self,
        major_version: u16,
        access_flags: u16,
        name: &str,
        super_name: Option<&str>,
        interfaces: &[&str],
    ) {
        self.cw
            .visit(major_version, 0, access_flags, name, super_name, interfaces);
    }

    pub fn visit_method(
        &mut self,
        access_flags: u16,
        name: &str,
        descriptor: &str,
    ) -> MethodWriter {
        let mv = self.cw.visit_method(access_flags, name, descriptor);
        MethodWriter {
            inner: mv,
            name: name.to_string(),
            descriptor: descriptor.to_string(),
            signature: None,
            exceptions: Vec::new(),
            local_variables: Vec::new(),
        }
    }

    pub fn visit_field(&mut self, access_flags: u16, name: &str, descriptor: &str) -> FieldWriter {
        let fv = self.cw.visit_field(access_flags, name, descriptor);
        FieldWriter {
            inner: fv,
            name: name.to_string(),
            descriptor: descriptor.to_string(),
            signature: None,
        }
    }

    pub fn visit_signature(&mut self, signature: &str) {
        self.class_signature = Some(signature.to_string());
    }

    pub fn visit_source_file(&mut self, name: &str) {
        self.cw.visit_source_file(name);
    }

    pub fn visit_nest_host(&mut self, host: &str) {
        self.nest_host = Some(host.to_string());
    }

    pub fn visit_nest_member(&mut self, member: &str) {
        self.nest_members.push(member.to_string());
    }

    pub fn to_bytes(self) -> Result<Vec<u8>, String> {
        let mut class_node = self.cw.to_class_node().map_err(|e| e.to_string())?;
        add_extra_attributes(
            &mut class_node,
            self.class_signature.as_deref(),
            self.nest_host.as_deref(),
            &self.nest_members,
            &self.field_metadata,
            &self.method_metadata,
        );

        let first_pass =
            AsmClassWriter::write_class_node(&class_node, COMPUTE_FRAMES | COMPUTE_MAXS)
                .map_err(|e| format!("{:?}", e))?;
        let code_lengths = method_code_lengths(&first_pass)?;
        add_local_variables(&mut class_node, &self.method_metadata, &code_lengths);

        AsmClassWriter::write_class_node(&class_node, COMPUTE_FRAMES | COMPUTE_MAXS)
            .map_err(|e| format!("{:?}", e))
    }
}

impl Default for ClassFileWriter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MethodWriter {
    inner: MethodVisitor,
    name: String,
    descriptor: String,
    signature: Option<String>,
    exceptions: Vec<String>,
    local_variables: Vec<LocalVariableSpec>,
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

    pub fn visit_new_array(&mut self, array_type: u8) {
        self.inner
            .visit_var_insn(rust_asm::opcodes::NEWARRAY, array_type as u16);
    }

    pub fn visit_jump_insn(&mut self, opcode: u8, target: Label) {
        self.inner.visit_jump_insn(opcode, target);
    }

    pub fn visit_lookup_switch(&mut self, default: Label, pairs: &[(i32, Label)]) {
        self.inner.visit_lookup_switch(default, pairs);
    }

    pub fn visit_label(&mut self, label: Label) {
        self.inner.visit_label(label);
    }

    pub fn visit_line_number(&mut self, line: u16, label: Label) {
        self.inner
            .visit_line_number(line, LabelNode::from_label(label));
    }

    pub fn visit_try_catch_block(
        &mut self,
        start: Label,
        end: Label,
        handler: Label,
        catch_type: Option<&str>,
    ) {
        self.inner
            .visit_try_catch_block(start, end, handler, catch_type);
    }

    pub fn visit_local_variable(&mut self, name: &str, descriptor: &str, index: u16) {
        self.local_variables.push(LocalVariableSpec {
            name: name.to_string(),
            descriptor: descriptor.to_string(),
            index,
        });
    }

    pub fn visit_signature(&mut self, signature: &str) {
        self.signature = Some(signature.to_string());
    }

    pub fn visit_exception(&mut self, internal_name: &str) {
        self.exceptions.push(internal_name.to_string());
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
        self.inner
            .visit_method_insn(opcode, owner, name, descriptor, is_interface);
    }

    pub fn visit_invoke_dynamic_insn(
        &mut self,
        name: &str,
        descriptor: &str,
        bootstrap_method: Handle,
        bootstrap_args: &[BootstrapArgument],
    ) {
        self.inner
            .visit_invokedynamic_insn(name, descriptor, bootstrap_method, bootstrap_args);
    }

    pub fn visit_ldc_insn_int(&mut self, value: i32) {
        self.inner
            .visit_ldc_insn(rust_asm::insn::LdcInsnNode::int(value));
    }

    pub fn visit_ldc_insn_float(&mut self, value: f32) {
        self.inner
            .visit_ldc_insn(rust_asm::insn::LdcInsnNode::float(value));
    }

    pub fn visit_ldc_insn_long(&mut self, value: i64) {
        self.inner
            .visit_ldc_insn(rust_asm::insn::LdcInsnNode::long(value));
    }

    pub fn visit_ldc_insn_double(&mut self, value: f64) {
        self.inner
            .visit_ldc_insn(rust_asm::insn::LdcInsnNode::double(value));
    }

    pub fn visit_ldc_insn_string(&mut self, value: &str) {
        self.inner
            .visit_ldc_insn(rust_asm::insn::LdcInsnNode::string(value));
    }

    pub fn visit_ldc_insn_type(&mut self, type_name: &str) {
        self.inner
            .visit_ldc_insn(rust_asm::insn::LdcInsnNode::typed(
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
        cw.method_metadata.push(MethodMetadata {
            name: self.name.clone(),
            descriptor: self.descriptor.clone(),
            signature: self.signature.clone(),
            exceptions: self.exceptions.clone(),
            local_variables: self.local_variables.clone(),
        });
        self.inner.visit_end(&mut cw.cw);
    }
}

pub struct FieldWriter {
    inner: FieldVisitor,
    name: String,
    descriptor: String,
    signature: Option<String>,
}

impl FieldWriter {
    pub fn visit_signature(&mut self, signature: &str) {
        self.signature = Some(signature.to_string());
    }

    pub fn visit_end(self, cw: &mut ClassFileWriter) {
        cw.field_metadata.push(FieldMetadata {
            name: self.name.clone(),
            descriptor: self.descriptor.clone(),
            signature: self.signature.clone(),
        });
        self.inner.visit_end(&mut cw.cw);
    }
}

#[derive(Debug, Clone)]
struct LocalVariableSpec {
    name: String,
    descriptor: String,
    index: u16,
}

#[derive(Debug, Clone)]
struct MethodMetadata {
    name: String,
    descriptor: String,
    signature: Option<String>,
    exceptions: Vec<String>,
    local_variables: Vec<LocalVariableSpec>,
}

#[derive(Debug, Clone)]
struct FieldMetadata {
    name: String,
    descriptor: String,
    signature: Option<String>,
}

fn add_extra_attributes(
    class_node: &mut rust_asm::nodes::ClassNode,
    class_signature: Option<&str>,
    nest_host: Option<&str>,
    nest_members: &[String],
    field_metadata: &[FieldMetadata],
    method_metadata: &[MethodMetadata],
) {
    let mut cp = ConstantPoolBuilder::from_pool(class_node.constant_pool.clone());
    if class_signature.is_some()
        || field_metadata
            .iter()
            .any(|metadata| metadata.signature.is_some())
        || method_metadata
            .iter()
            .any(|metadata| metadata.signature.is_some())
    {
        cp.utf8("Signature");
    }
    if method_metadata
        .iter()
        .any(|metadata| !metadata.exceptions.is_empty())
    {
        cp.utf8("Exceptions");
    }
    if nest_host.is_some() {
        cp.utf8("NestHost");
    }
    if !nest_members.is_empty() {
        cp.utf8("NestMembers");
    }
    for metadata in field_metadata {
        cp.utf8(&metadata.name);
        cp.utf8(&metadata.descriptor);
    }
    for metadata in method_metadata {
        cp.utf8(&metadata.name);
        cp.utf8(&metadata.descriptor);
    }

    if let Some(signature) = class_signature {
        add_signature_attribute(&mut class_node.attributes, &mut cp, signature);
    }
    if let Some(host) = nest_host {
        add_nest_host_attribute(&mut class_node.attributes, &mut cp, host);
    }
    if !nest_members.is_empty() {
        add_nest_members_attribute(&mut class_node.attributes, &mut cp, nest_members);
    }

    for (field, metadata) in class_node.fields.iter_mut().zip(field_metadata) {
        if field.name == metadata.name
            && field.descriptor == metadata.descriptor
            && let Some(signature) = metadata.signature.as_deref()
        {
            add_signature_attribute(&mut field.attributes, &mut cp, signature);
        }
    }

    for (method, metadata) in class_node.methods.iter_mut().zip(method_metadata) {
        if method.name == metadata.name
            && method.descriptor == metadata.descriptor
            && let Some(signature) = metadata.signature.as_deref()
        {
            add_signature_attribute(&mut method.attributes, &mut cp, signature);
        }
        if method.name == metadata.name
            && method.descriptor == metadata.descriptor
            && !metadata.exceptions.is_empty()
        {
            add_exceptions_attribute(&mut method.attributes, &mut cp, &metadata.exceptions);
        }
    }

    class_node.constant_pool = cp.into_pool();
}

fn add_nest_host_attribute(
    attributes: &mut Vec<AttributeInfo>,
    cp: &mut ConstantPoolBuilder,
    host: &str,
) {
    attributes
        .retain(|attr| !matches!(attr, AttributeInfo::Unknown { name, .. } if name == "NestHost"));
    attributes.push(AttributeInfo::Unknown {
        name: "NestHost".to_string(),
        info: cp.class(host).to_be_bytes().to_vec(),
    });
}

fn add_nest_members_attribute(
    attributes: &mut Vec<AttributeInfo>,
    cp: &mut ConstantPoolBuilder,
    members: &[String],
) {
    attributes.retain(
        |attr| !matches!(attr, AttributeInfo::Unknown { name, .. } if name == "NestMembers"),
    );
    let mut info = Vec::new();
    info.extend_from_slice(&(members.len() as u16).to_be_bytes());
    for member in members {
        info.extend_from_slice(&cp.class(member).to_be_bytes());
    }
    attributes.push(AttributeInfo::Unknown {
        name: "NestMembers".to_string(),
        info,
    });
}

fn add_signature_attribute(
    attributes: &mut Vec<AttributeInfo>,
    cp: &mut ConstantPoolBuilder,
    signature: &str,
) {
    attributes.retain(|attr| !matches!(attr, AttributeInfo::Signature { .. }));
    let signature_index = cp.utf8(signature);
    attributes.push(AttributeInfo::Signature { signature_index });
}

fn add_exceptions_attribute(
    attributes: &mut Vec<AttributeInfo>,
    cp: &mut ConstantPoolBuilder,
    exceptions: &[String],
) {
    attributes.retain(|attr| !matches!(attr, AttributeInfo::Exceptions { .. }));
    let exception_index_table = exceptions
        .iter()
        .map(|exception| cp.class(exception))
        .collect();
    attributes.push(AttributeInfo::Exceptions {
        exception_index_table,
    });
}

fn add_local_variables(
    class_node: &mut rust_asm::nodes::ClassNode,
    method_metadata: &[MethodMetadata],
    code_lengths: &HashMap<(String, String), u16>,
) {
    let mut cp = ConstantPoolBuilder::from_pool(class_node.constant_pool.clone());
    cp.utf8("LocalVariableTable");

    for (method, metadata) in class_node.methods.iter_mut().zip(method_metadata) {
        if method.name != metadata.name || method.descriptor != metadata.descriptor {
            continue;
        }
        let Some(length) = code_lengths
            .get(&(metadata.name.clone(), metadata.descriptor.clone()))
            .copied()
            .filter(|length| *length > 0)
        else {
            continue;
        };

        method.local_variables.clear();
        for variable in &metadata.local_variables {
            method.local_variables.push(LocalVariable {
                start_pc: 0,
                length,
                name_index: cp.utf8(&variable.name),
                descriptor_index: cp.utf8(&variable.descriptor),
                index: variable.index,
            });
        }
    }

    class_node.constant_pool = cp.into_pool();
}

fn method_code_lengths(bytes: &[u8]) -> Result<HashMap<(String, String), u16>, String> {
    let class_file = read_class_file(bytes).map_err(|e| format!("{:?}", e))?;
    let mut lengths = HashMap::new();

    for method in class_file.methods {
        let name = cp_utf8(&class_file.constant_pool, method.name_index)?.to_string();
        let descriptor = cp_utf8(&class_file.constant_pool, method.descriptor_index)?.to_string();
        let length = method
            .attributes
            .iter()
            .find_map(|attr| match attr {
                AttributeInfo::Code(code) => Some(code.code.len().min(u16::MAX as usize) as u16),
                _ => None,
            })
            .unwrap_or(0);
        lengths.insert((name, descriptor), length);
    }

    Ok(lengths)
}

fn cp_utf8(pool: &[CpInfo], index: u16) -> Result<&str, String> {
    match pool.get(index as usize) {
        Some(CpInfo::Utf8(value)) => Ok(value.as_str()),
        _ => Err(format!("invalid UTF-8 constant pool index {index}")),
    }
}
