use crate::bytecode::codegen::CodegenCtx;
use crate::bytecode::expr_gen;
use crate::bytecode::local_var::return_opcode;
use crate::call_resolver::{ClassCatalog, MethodRef};
use crate::classfile::{ClassFileWriter, MethodWriter};
use crate::hir::infer::{self, TypeEnvironment};
use crate::hir::{Expr, ExprId, LambdaBody, LambdaParam, MethodDecl, TypeDecl};
use crate::ty::MethodSig;
use crate::ty::Ty;
use rust_asm::insn::{BootstrapArgument, Handle};
use std::collections::HashMap;

const LAMBDA_METAFACTORY: &str = "java/lang/invoke/LambdaMetafactory";
const METAFACTORY_NAME: &str = "metafactory";
const METAFACTORY_DESC: &str = "(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodHandle;Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;";
const OBJECT_DESCRIPTOR: &str = "Ljava/lang/Object;";
const SUPPLIER_INTERFACE: &str = "java/util/function/Supplier";
const FUNCTION_INTERFACE: &str = "java/util/function/Function";
const BIFUNCTION_INTERFACE: &str = "java/util/function/BiFunction";

pub(crate) type LambdaTable = HashMap<ExprId, LambdaInfo>;

#[derive(Clone)]
pub(crate) struct LambdaInfo {
    pub sam_method_name: String,
    pub sam_method_type: String,
    pub factory_method_descriptor: String,
    pub synthetic_method_descriptor: String,
    pub synthetic_method_handle: Handle,
}

pub(crate) fn emit_lambda_methods(
    writer: &mut ClassFileWriter,
    type_decl: &TypeDecl,
    super_name: &str,
    catalog: &ClassCatalog,
    method: &MethodDecl,
    counter: &mut u32,
) -> LambdaTable {
    let mut lambdas = LambdaTable::new();
    for (expr_id, expr) in method.body.exprs.iter() {
        let Expr::Lambda { params, body, .. } = expr else {
            continue;
        };

        let target = FunctionalInterfaceTarget::from_expr(expr, catalog, params.len());
        let synthetic_name = synthetic_lambda_name(method, counter);
        let synthetic_return_type = synthetic_lambda_return_type(
            &target.return_type,
            inferred_lambda_return_type(type_decl, super_name, catalog, method, params, body),
        );
        let synthetic_method_descriptor =
            synthetic_method_descriptor(params, &synthetic_return_type);
        let factory_method_descriptor = format!("()L{};", target.interface);
        let synthetic_method_handle = Handle {
            reference_kind: rust_asm::constants::REF_INVOKE_STATIC,
            owner: type_decl.name.to_string(),
            name: synthetic_name.clone(),
            descriptor: synthetic_method_descriptor.clone(),
            is_interface: false,
        };

        emit_synthetic_lambda_method(
            writer,
            type_decl,
            super_name,
            catalog,
            method,
            LambdaMethod {
                name: &synthetic_name,
                descriptor: &synthetic_method_descriptor,
                return_type: &synthetic_return_type,
                params,
                body,
            },
        );

        lambdas.insert(
            expr_id,
            LambdaInfo {
                sam_method_name: target.method_name,
                sam_method_type: target.method_type,
                factory_method_descriptor,
                synthetic_method_descriptor,
                synthetic_method_handle,
            },
        );
    }
    lambdas
}

pub(crate) fn emit_invokedynamic(mw: &mut MethodWriter, ctx: &CodegenCtx, expr_id: ExprId) {
    let Some(info) = ctx.lambdas.get(&expr_id) else {
        mw.visit_insn(rust_asm::opcodes::ACONST_NULL);
        return;
    };

    let bootstrap_method = Handle {
        reference_kind: rust_asm::constants::REF_INVOKE_STATIC,
        owner: LAMBDA_METAFACTORY.to_string(),
        name: METAFACTORY_NAME.to_string(),
        descriptor: METAFACTORY_DESC.to_string(),
        is_interface: false,
    };
    let bootstrap_args = vec![
        BootstrapArgument::MethodType(info.sam_method_type.clone()),
        BootstrapArgument::Handle(info.synthetic_method_handle.clone()),
        BootstrapArgument::MethodType(info.synthetic_method_descriptor.clone()),
    ];

    mw.visit_invoke_dynamic_insn(
        &info.sam_method_name,
        &info.factory_method_descriptor,
        bootstrap_method,
        &bootstrap_args,
    );
}

struct FunctionalInterfaceTarget {
    interface: String,
    method_name: String,
    method_type: String,
    return_type: Ty,
}

impl FunctionalInterfaceTarget {
    fn from_expr(expr: &Expr, catalog: &ClassCatalog, param_count: usize) -> Self {
        if let Expr::Lambda {
            target_ty: Some(Ty::Class(name)),
            ..
        } = expr
            && let Some(method) = catalog.functional_interface_method(name)
        {
            return Self::from_functional_method(name.as_str(), method);
        }

        Self::fallback_for_param_count(param_count)
    }

    fn from_functional_method(interface: &str, method: MethodRef) -> Self {
        let method_type = erased_sam_method_type(&method);
        Self {
            interface: interface.to_string(),
            method_name: method.name,
            method_type,
            return_type: method.return_ty.erasure(),
        }
    }

    fn fallback_for_param_count(param_count: usize) -> Self {
        match param_count {
            0 => Self {
                interface: SUPPLIER_INTERFACE.to_string(),
                method_name: "get".to_string(),
                method_type: format!("(){}", OBJECT_DESCRIPTOR),
                return_type: Ty::object(),
            },
            1 => Self {
                interface: FUNCTION_INTERFACE.to_string(),
                method_name: "apply".to_string(),
                method_type: format!("({}){}", OBJECT_DESCRIPTOR, OBJECT_DESCRIPTOR),
                return_type: Ty::object(),
            },
            _ => Self {
                interface: BIFUNCTION_INTERFACE.to_string(),
                method_name: "apply".to_string(),
                method_type: format!(
                    "({}{}){}",
                    OBJECT_DESCRIPTOR, OBJECT_DESCRIPTOR, OBJECT_DESCRIPTOR
                ),
                return_type: Ty::object(),
            },
        }
    }
}

struct LambdaMethod<'a> {
    name: &'a str,
    descriptor: &'a str,
    return_type: &'a Ty,
    params: &'a [LambdaParam],
    body: &'a LambdaBody,
}

fn emit_synthetic_lambda_method(
    writer: &mut ClassFileWriter,
    type_decl: &TypeDecl,
    super_name: &str,
    catalog: &ClassCatalog,
    owner_method: &MethodDecl,
    lambda: LambdaMethod<'_>,
) {
    let mut mw = writer.visit_method(
        crate::classfile::ACC_PRIVATE
            | crate::classfile::ACC_STATIC
            | crate::classfile::ACC_SYNTHETIC,
        lambda.name,
        lambda.descriptor,
    );
    mw.visit_code();

    let mut ctx = CodegenCtx::new(writer, type_decl.name, catalog);
    ctx.set_super_name(ustr::Ustr::from(super_name));
    ctx.set_fields(&type_decl.fields);
    ctx.set_methods(&type_decl.methods);
    begin_lambda_method(&mut mw, &mut ctx, lambda.return_type, lambda.params);

    match lambda.body {
        LambdaBody::Expr(body_expr_id) => {
            expr_gen::gen_expr(&mut mw, &mut ctx, &owner_method.body, *body_expr_id);
            let body_type = expr_gen::expr_ty(&ctx, &owner_method.body, *body_expr_id);
            emit_lambda_return(&mut mw, &body_type, lambda.return_type);
        }
        LambdaBody::Block(block) => {
            crate::bytecode::method_gen::gen_method_body(
                &mut mw,
                &mut ctx,
                &owner_method.body,
                block,
            );
        }
    }

    mw.visit_maxs(0, 0);
    mw.visit_end(writer);
}

fn emit_lambda_return(mw: &mut MethodWriter, body_type: &Ty, return_type: &Ty) {
    if matches!(return_type, Ty::Void) {
        expr_gen::pop_ty(mw, body_type);
        mw.visit_insn(rust_asm::opcodes::RETURN);
        return;
    }

    expr_gen::coerce(mw, body_type, return_type);
    mw.visit_insn(return_opcode(return_type));
}

fn synthetic_lambda_return_type(target_return_type: &Ty, inferred_return_type: Option<Ty>) -> Ty {
    if matches!(target_return_type, Ty::Void) {
        return Ty::Void;
    }

    inferred_return_type.unwrap_or_else(|| target_return_type.clone())
}

fn inferred_lambda_return_type(
    type_decl: &TypeDecl,
    super_name: &str,
    catalog: &ClassCatalog,
    owner_method: &MethodDecl,
    params: &[LambdaParam],
    body: &LambdaBody,
) -> Option<Ty> {
    let mut local_types = HashMap::new();
    for param in params {
        local_types.insert(param.name, param.ty.clone().unwrap_or_else(Ty::object));
    }

    let env = LambdaTypeEnv {
        catalog,
        class_name: type_decl.name,
        super_name: ustr::Ustr::from(super_name),
        local_types,
        fields: type_decl
            .fields
            .iter()
            .map(|field| (field.name, field.ty.clone()))
            .collect(),
        methods: type_decl
            .methods
            .iter()
            .map(|method| {
                let mut sig = method.signature.clone();
                sig.access_flags = method.access_flags;
                (method.name, sig)
            })
            .collect(),
    };
    let inferred = infer::lambda_body_ty(&env, &owner_method.body, body);
    (!matches!(inferred, Ty::Void)).then_some(inferred)
}

struct LambdaTypeEnv<'a> {
    catalog: &'a ClassCatalog,
    class_name: ustr::Ustr,
    super_name: ustr::Ustr,
    local_types: HashMap<ustr::Ustr, Ty>,
    fields: HashMap<ustr::Ustr, Ty>,
    methods: HashMap<ustr::Ustr, MethodSig>,
}

impl TypeEnvironment for LambdaTypeEnv<'_> {
    fn local_ty(&self, name: ustr::Ustr) -> Option<Ty> {
        self.local_types.get(&name).cloned()
    }

    fn field_ty(&self, name: ustr::Ustr) -> Option<Ty> {
        self.fields.get(&name).cloned()
    }

    fn resolve_static_field(&self, owner: &str, name: &str) -> Option<Ty> {
        self.catalog
            .resolve_static_field(owner, name)
            .map(|field| field.ty)
    }

    fn resolve_instance_method(&self, receiver: &Ty, name: &str, args: &[Ty]) -> Option<Ty> {
        self.catalog
            .resolve_instance_method(receiver, name, args)
            .map(|method| method.return_ty)
    }

    fn resolve_static_method(&self, owner: &str, name: &str, args: &[Ty]) -> Option<Ty> {
        self.catalog
            .resolve_static_method(owner, name, args)
            .map(|method| method.return_ty)
    }

    fn resolve_current_method(&self, name: ustr::Ustr, _args: &[Ty]) -> Option<Ty> {
        self.methods.get(&name).map(|sig| sig.return_type.clone())
    }

    fn this_ty(&self) -> Ty {
        Ty::Class(self.class_name)
    }

    fn super_ty(&self) -> Ty {
        Ty::Class(self.super_name)
    }
}

fn begin_lambda_method(
    mw: &mut MethodWriter,
    ctx: &mut CodegenCtx,
    return_type: &Ty,
    params: &[LambdaParam],
) {
    ctx.return_ty = return_type.clone();
    ctx.next_local = 0;
    ctx.locals.clear();
    ctx.local_types.clear();

    for param in params {
        let ty = param.ty.clone().unwrap_or_else(Ty::object);
        let slot = ctx.next_local;
        mw.visit_local_variable(param.name.as_str(), &ty.erasure().descriptor(), slot);
        ctx.locals.insert(param.name, slot);
        ctx.local_types.insert(param.name, ty.clone());
        ctx.next_local += ty.size() as u16;
    }
}

fn synthetic_lambda_name(method: &MethodDecl, counter: &mut u32) -> String {
    let name = format!("lambda${}${}", method.name, *counter);
    *counter += 1;
    name
}

fn synthetic_method_descriptor(params: &[LambdaParam], return_type: &Ty) -> String {
    format!(
        "({}){}",
        erased_param_descriptors(params),
        return_type.descriptor()
    )
}

fn erased_sam_method_type(method: &MethodRef) -> String {
    format!(
        "({}){}",
        method_param_descriptors(&method.params),
        method.return_ty.erasure().descriptor()
    )
}

fn erased_param_descriptors(params: &[LambdaParam]) -> String {
    params
        .iter()
        .map(|param| {
            param
                .ty
                .clone()
                .unwrap_or_else(Ty::object)
                .erasure()
                .descriptor()
        })
        .collect()
}

fn method_param_descriptors(params: &[Ty]) -> String {
    params
        .iter()
        .map(|param| param.erasure().descriptor())
        .collect()
}
