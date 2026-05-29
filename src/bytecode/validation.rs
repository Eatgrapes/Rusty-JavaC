#[path = "validation/diagnostic.rs"]
mod diagnostic;
#[path = "validation/scope.rs"]
mod scope;

use crate::bytecode::error::BytecodeError;
use crate::call_resolver::ClassCatalog;
use crate::hir::infer::{self, TypeEnvironment};
use crate::hir::*;
use crate::ty::{MethodSig, Ty};
use diagnostic::{
    display_internal_name, invalid_this_method_receiver, unresolved_field, unresolved_method,
    unresolved_variable,
};
use scope::MethodScope;
use std::collections::HashMap;
use ustr::Ustr;

type ValidateResult<T> = Result<T, BytecodeError>;

pub(crate) fn validate_type_decl(
    type_decl: &TypeDecl,
    catalog: &ClassCatalog,
) -> ValidateResult<()> {
    let validator = Validator::new(type_decl, catalog);

    for field in &type_decl.fields {
        validator.validate_field(field)?;
    }
    for method in &type_decl.methods {
        validator.validate_method(method)?;
    }

    Ok(())
}

struct Validator {
    catalog: ClassCatalog,
    class_name: Ustr,
    super_name: Ustr,
    fields: HashMap<Ustr, FieldInfo>,
    outer_fields: HashMap<Ustr, FieldInfo>,
    enclosing_static_owner: Option<Ustr>,
    methods: HashMap<Ustr, MethodSig>,
}

#[derive(Clone)]
struct FieldInfo {
    ty: Ty,
    access_flags: u16,
}

impl Validator {
    fn new(type_decl: &TypeDecl, catalog: &ClassCatalog) -> Self {
        let fields = type_decl
            .fields
            .iter()
            .map(|field| {
                (
                    field.name,
                    FieldInfo {
                        ty: field.ty.clone(),
                        access_flags: field.access_flags,
                    },
                )
            })
            .collect();
        let methods = type_decl
            .methods
            .iter()
            .map(|method| {
                let mut sig = method.signature.clone();
                sig.access_flags = method.access_flags;
                (method.name, sig)
            })
            .collect();
        let outer_fields = type_decl
            .anonymous
            .as_ref()
            .map(|info| {
                info.outer_fields
                    .iter()
                    .map(|field| {
                        (
                            field.name,
                            FieldInfo {
                                ty: field.ty.clone(),
                                access_flags: field.access_flags,
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self {
            catalog: catalog.clone(),
            class_name: type_decl.name,
            super_name: type_decl
                .super_class
                .as_ref()
                .map(|ty| Ustr::from(&ty.internal_name()))
                .unwrap_or_else(|| Ustr::from("java/lang/Object")),
            fields,
            outer_fields,
            enclosing_static_owner: type_decl
                .anonymous
                .as_ref()
                .and_then(|info| info.enclosing_static_owner),
            methods,
        }
    }

    fn validate_field(&self, field: &FieldDecl) -> ValidateResult<()> {
        let mut scope = MethodScope::default();
        if let Some(initializer) = field.initializer {
            self.validate_expr(&field.body, &mut scope, initializer)?;
        }
        Ok(())
    }

    fn validate_method(&self, method: &MethodDecl) -> ValidateResult<()> {
        let mut scope = MethodScope::default();
        for param in &method.params {
            scope.locals.insert(param.name, param.ty.clone());
        }
        if let Some(block) = &method.root_block {
            self.validate_block(&method.body, &mut scope, block)?;
        }
        Ok(())
    }

    fn validate_block(
        &self,
        body: &Body,
        scope: &mut MethodScope,
        block: &Block,
    ) -> ValidateResult<()> {
        for stmt in &block.stmts {
            self.validate_stmt(body, scope, *stmt)?;
        }
        Ok(())
    }

    fn validate_stmt(
        &self,
        body: &Body,
        scope: &mut MethodScope,
        stmt_id: StmtId,
    ) -> ValidateResult<()> {
        let line = body.stmt_lines.get(&stmt_id).copied().or(scope.line);
        let mut stmt_scope = scope.with_line(line);

        match &body.stmts[stmt_id] {
            Stmt::Expr(expr) | Stmt::Throw(expr) | Stmt::Yield(expr) => {
                self.validate_expr(body, &mut stmt_scope, *expr)
            }
            Stmt::Return(Some(expr)) => self.validate_expr(body, &mut stmt_scope, *expr),
            Stmt::Return(None) | Stmt::Empty | Stmt::Break(_) | Stmt::Continue(_) => Ok(()),
            Stmt::LocalVar(var) => {
                if let Some(initializer) = var.initializer {
                    self.validate_expr(body, &mut stmt_scope, initializer)?;
                }
                scope.locals.insert(var.name, var.ty.clone());
                Ok(())
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.validate_expr(body, &mut stmt_scope, *condition)?;
                let mut then_scope = scope.clone();
                if let Some((name, ty)) = pattern_binding(body, *condition) {
                    then_scope.locals.insert(name, ty);
                }
                self.validate_stmt(body, &mut then_scope, *then_branch)?;
                if let Some(else_branch) = else_branch {
                    self.validate_stmt(body, &mut scope.clone(), *else_branch)?;
                }
                Ok(())
            }
            Stmt::For {
                init,
                condition,
                update,
                body: loop_body,
            } => {
                let mut loop_scope = scope.clone();
                if let Some(init) = init {
                    self.validate_stmt(body, &mut loop_scope, *init)?;
                }
                loop_scope.line = line;
                if let Some(condition) = condition {
                    self.validate_expr(body, &mut loop_scope, *condition)?;
                }
                if let Some(update) = update {
                    self.validate_expr(body, &mut loop_scope, *update)?;
                }
                self.validate_stmt(body, &mut loop_scope, *loop_body)
            }
            Stmt::ForEach {
                var_type,
                var_name,
                iterable,
                body: loop_body,
            } => {
                let mut loop_scope = scope.clone();
                loop_scope.line = line;
                self.validate_expr(body, &mut loop_scope, *iterable)?;
                loop_scope.locals.insert(*var_name, var_type.clone());
                self.validate_stmt(body, &mut loop_scope, *loop_body)
            }
            Stmt::While {
                condition,
                body: loop_body,
            } => {
                self.validate_expr(body, &mut stmt_scope, *condition)?;
                self.validate_stmt(body, &mut scope.clone(), *loop_body)
            }
            Stmt::Do {
                body: loop_body,
                condition,
            } => {
                self.validate_stmt(body, &mut scope.clone(), *loop_body)?;
                self.validate_expr(body, &mut stmt_scope, *condition)
            }
            Stmt::Labeled {
                body: labeled_body, ..
            } => self.validate_stmt(body, &mut scope.clone(), *labeled_body),
            Stmt::Switch { selector, cases } => {
                self.validate_expr(body, &mut stmt_scope, *selector)?;
                for case in cases {
                    if let SwitchCase::Case { pattern, .. } = case {
                        self.validate_expr(body, &mut stmt_scope, *pattern)?;
                    }
                    let mut case_scope = scope.clone();
                    for stmt in case_stmts(case) {
                        self.validate_stmt(body, &mut case_scope, *stmt)?;
                    }
                }
                Ok(())
            }
            Stmt::Try(try_stmt) => {
                let mut try_scope = scope.clone();
                try_scope.line = line;
                for resource in &try_stmt.resources {
                    if let Some(initializer) = resource.initializer {
                        self.validate_expr(body, &mut try_scope, initializer)?;
                    }
                    try_scope.locals.insert(resource.name, resource.ty.clone());
                }
                self.validate_block(body, &mut try_scope, &try_stmt.body)?;
                for catch in &try_stmt.catches {
                    let mut catch_scope = scope.clone();
                    catch_scope
                        .locals
                        .insert(catch.var_name, catch.exception_type.clone());
                    self.validate_block(body, &mut catch_scope, &catch.body)?;
                }
                if let Some(finally) = &try_stmt.finally {
                    self.validate_block(body, &mut scope.clone(), finally)?;
                }
                Ok(())
            }
            Stmt::Synchronized(expr, block) => {
                self.validate_expr(body, &mut stmt_scope, *expr)?;
                self.validate_block(body, &mut scope.clone(), block)
            }
            Stmt::Assert { condition, message } => {
                self.validate_expr(body, &mut stmt_scope, *condition)?;
                if let Some(message) = message {
                    self.validate_expr(body, &mut stmt_scope, *message)?;
                }
                Ok(())
            }
            Stmt::Block(block) => self.validate_block(body, &mut scope.clone(), block),
        }
    }

    fn validate_expr(
        &self,
        body: &Body,
        scope: &mut MethodScope,
        expr_id: ExprId,
    ) -> ValidateResult<()> {
        match &body.exprs[expr_id] {
            Expr::FieldAccess { target, field } => {
                self.validate_receiver_expr(body, scope, *target)?;
                self.validate_field_access(body, scope, *target, *field)
            }
            Expr::MethodCall {
                target,
                method,
                args,
            } => {
                if let Some(target) = target {
                    self.validate_receiver_expr(body, scope, *target)?;
                }
                for arg in args {
                    self.validate_expr(body, scope, *arg)?;
                }
                self.validate_method_call(body, scope, *target, *method, args)
            }
            Expr::NewObject { args, .. } => {
                for arg in args {
                    self.validate_expr(body, scope, *arg)?;
                }
                Ok(())
            }
            Expr::NewArray {
                dimensions,
                initializer,
                ..
            } => {
                for dimension in dimensions.iter().flatten() {
                    self.validate_expr(body, scope, *dimension)?;
                }
                if let Some(initializer) = initializer {
                    for element in &initializer.elements {
                        self.validate_expr(body, scope, *element)?;
                    }
                }
                Ok(())
            }
            Expr::ArrayAccess { array, index } => {
                self.validate_expr(body, scope, *array)?;
                self.validate_expr(body, scope, *index)
            }
            Expr::Unary { operand, .. }
            | Expr::PostInc(operand)
            | Expr::PostDec(operand)
            | Expr::Parens(operand)
            | Expr::Cast { expr: operand, .. }
            | Expr::Instanceof { expr: operand, .. } => self.validate_expr(body, scope, *operand),
            Expr::Binary { left, right, .. }
            | Expr::Assign {
                target: left,
                value: right,
                ..
            } => {
                self.validate_expr(body, scope, *left)?;
                self.validate_expr(body, scope, *right)
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.validate_expr(body, scope, *condition)?;
                self.validate_expr(body, scope, *then_expr)?;
                self.validate_expr(body, scope, *else_expr)
            }
            Expr::Switch {
                selector, cases, ..
            } => {
                self.validate_expr(body, scope, *selector)?;
                for case in cases {
                    if let SwitchCase::Case { pattern, .. } = case {
                        self.validate_expr(body, scope, *pattern)?;
                    }
                    let mut case_scope = scope.clone();
                    for stmt in case_stmts(case) {
                        self.validate_stmt(body, &mut case_scope, *stmt)?;
                    }
                }
                Ok(())
            }
            Expr::Lambda {
                params,
                body: lambda,
                ..
            } => {
                let mut lambda_scope = scope.clone();
                for param in params {
                    lambda_scope
                        .locals
                        .insert(param.name, param.ty.clone().unwrap_or(Ty::object()));
                }
                match lambda {
                    LambdaBody::Expr(expr) => self.validate_expr(body, &mut lambda_scope, *expr),
                    LambdaBody::Block(block) => self.validate_block(body, &mut lambda_scope, block),
                }
            }
            Expr::MethodRef { target, .. } => self.validate_expr(body, scope, *target),
            Expr::IntLiteral(_)
            | Expr::LongLiteral(_)
            | Expr::FloatLiteral(_)
            | Expr::DoubleLiteral(_)
            | Expr::BoolLiteral(_)
            | Expr::CharLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::NullLiteral
            | Expr::This
            | Expr::Super
            | Expr::ClassName(_) => Ok(()),
            Expr::Ident(name) => self.validate_identifier(scope, *name),
        }
    }

    fn validate_receiver_expr(
        &self,
        body: &Body,
        scope: &mut MethodScope,
        expr_id: ExprId,
    ) -> ValidateResult<()> {
        if static_class_name(body, expr_id).is_some() {
            return Ok(());
        }
        self.validate_expr(body, scope, expr_id)
    }

    fn validate_identifier(&self, scope: &MethodScope, name: Ustr) -> ValidateResult<()> {
        if scope.locals.contains_key(&name)
            || self.fields.contains_key(&name)
            || self.outer_fields.contains_key(&name)
            || self.enclosing_static_owner.is_some_and(|owner| {
                self.catalog
                    .resolve_static_field(owner.as_str(), name.as_str())
                    .is_some()
            })
        {
            return Ok(());
        }
        Err(unresolved_variable(name, scope.line))
    }

    fn validate_field_access(
        &self,
        body: &Body,
        scope: &MethodScope,
        target: ExprId,
        field: Ustr,
    ) -> ValidateResult<()> {
        if field.as_str() == "length"
            && matches!(self.expr_ty(body, scope, target).erasure(), Ty::Array(_))
        {
            return Ok(());
        }
        if let Some(owner) = static_class_name(body, target) {
            if !self.has_static_field(owner, field) {
                return Err(unresolved_field(
                    field,
                    &display_internal_name(owner),
                    scope.line,
                ));
            }
            return Ok(());
        }
        if is_current_instance(body, target) {
            if self.fields.contains_key(&field) {
                return Ok(());
            }
            return Err(unresolved_field(
                field,
                &display_internal_name(self.class_name.as_str()),
                scope.line,
            ));
        }

        let receiver = self.expr_ty(body, scope, target);
        if self.is_current_class_ty(&receiver) && self.fields.contains_key(&field) {
            return Ok(());
        }
        Err(unresolved_field(field, &receiver.to_string(), scope.line))
    }

    fn validate_method_call(
        &self,
        body: &Body,
        scope: &MethodScope,
        target: Option<ExprId>,
        method: Ustr,
        args: &[ExprId],
    ) -> ValidateResult<()> {
        let arg_types = args
            .iter()
            .map(|arg| self.expr_ty(body, scope, *arg))
            .collect::<Vec<_>>();

        if let Some(target) = target {
            if let Some(owner) = static_class_name(body, target) {
                if self
                    .catalog
                    .resolve_static_method(owner, method.as_str(), &arg_types)
                    .is_some()
                    || (owner == self.class_name.as_str()
                        && self.methods.get(&method).is_some_and(|sig| {
                            sig.access_flags & crate::classfile::ACC_STATIC != 0
                        }))
                {
                    return Ok(());
                }
                return Err(unresolved_method(
                    method,
                    &arg_types,
                    &display_internal_name(owner),
                    scope.line,
                ));
            }

            let receiver = self.expr_ty(body, scope, target);
            if self.is_current_class_ty(&receiver)
                && (self.methods.contains_key(&method)
                    || self
                        .catalog
                        .resolve_instance_method(
                            &Ty::Class(self.super_name),
                            method.as_str(),
                            &arg_types,
                        )
                        .is_some())
            {
                return Ok(());
            }
            if self
                .catalog
                .resolve_instance_method(&receiver, method.as_str(), &arg_types)
                .is_some()
            {
                return Ok(());
            }

            if is_current_instance(body, target) {
                return self.validate_current_class_call(method, &arg_types, false, scope.line);
            }

            return Err(unresolved_method(
                method,
                &arg_types,
                &receiver.to_string(),
                scope.line,
            ));
        }

        self.validate_current_class_call(method, &arg_types, true, scope.line)
    }

    fn validate_current_class_call(
        &self,
        method: Ustr,
        arg_types: &[Ty],
        allow_static: bool,
        line: Option<u16>,
    ) -> ValidateResult<()> {
        let Some(sig) = self.methods.get(&method) else {
            if self
                .catalog
                .resolve_instance_method(&Ty::Class(self.super_name), method.as_str(), arg_types)
                .is_some()
            {
                return Ok(());
            }
            if allow_static
                && let Some(owner) = self.enclosing_static_owner
                && self
                    .catalog
                    .resolve_static_method(owner.as_str(), method.as_str(), arg_types)
                    .is_some()
            {
                return Ok(());
            }
            return Err(unresolved_method(
                method,
                arg_types,
                &display_internal_name(self.class_name.as_str()),
                line,
            ));
        };
        let is_static = sig.access_flags & crate::classfile::ACC_STATIC != 0;
        if is_static && !allow_static {
            return Err(invalid_this_method_receiver(method, line));
        }
        Ok(())
    }

    fn has_static_field(&self, owner: &str, field: Ustr) -> bool {
        self.catalog
            .resolve_static_field(owner, field.as_str())
            .is_some()
            || (owner == self.class_name.as_str()
                && self
                    .fields
                    .get(&field)
                    .is_some_and(|info| info.access_flags & crate::classfile::ACC_STATIC != 0))
    }

    fn current_static_field_ty(&self, owner: &str, field: &str) -> Option<Ty> {
        if owner != self.class_name.as_str() {
            return None;
        }
        self.fields
            .get(&Ustr::from(field))
            .filter(|info| info.access_flags & crate::classfile::ACC_STATIC != 0)
            .map(|info| info.ty.clone())
    }

    fn is_current_class_ty(&self, ty: &Ty) -> bool {
        matches!(ty.erasure(), Ty::Class(name) if name == self.class_name)
    }

    fn expr_ty(&self, body: &Body, scope: &MethodScope, expr_id: ExprId) -> Ty {
        infer::expr_ty(
            &ValidationTypeEnv {
                validator: self,
                scope,
            },
            body,
            expr_id,
        )
    }
}

struct ValidationTypeEnv<'a> {
    validator: &'a Validator,
    scope: &'a MethodScope,
}

impl TypeEnvironment for ValidationTypeEnv<'_> {
    fn local_ty(&self, name: Ustr) -> Option<Ty> {
        self.scope.locals.get(&name).cloned()
    }

    fn field_ty(&self, name: Ustr) -> Option<Ty> {
        self.validator
            .fields
            .get(&name)
            .map(|field| field.ty.clone())
            .or_else(|| {
                self.validator
                    .outer_fields
                    .get(&name)
                    .map(|field| field.ty.clone())
            })
            .or_else(|| {
                self.validator.enclosing_static_owner.and_then(|owner| {
                    self.validator
                        .catalog
                        .resolve_static_field(owner.as_str(), name.as_str())
                        .map(|field| field.ty)
                })
            })
    }

    fn resolve_static_field(&self, owner: &str, name: &str) -> Option<Ty> {
        self.validator
            .catalog
            .resolve_static_field(owner, name)
            .map(|field| field.ty)
            .or_else(|| self.validator.current_static_field_ty(owner, name))
    }

    fn resolve_instance_method(&self, receiver: &Ty, name: &str, args: &[Ty]) -> Option<Ty> {
        if self.validator.is_current_class_ty(receiver) {
            if let Some(sig) = self.validator.methods.get(&Ustr::from(name))
                && sig.access_flags & crate::classfile::ACC_STATIC == 0
            {
                return Some(sig.return_type.clone());
            }
            return self
                .validator
                .catalog
                .resolve_instance_method(&Ty::Class(self.validator.super_name), name, args)
                .map(|method| method.return_ty);
        }
        self.validator
            .catalog
            .resolve_instance_method(receiver, name, args)
            .map(|method| method.return_ty)
    }

    fn resolve_static_method(&self, owner: &str, name: &str, args: &[Ty]) -> Option<Ty> {
        self.validator
            .catalog
            .resolve_static_method(owner, name, args)
            .map(|method| method.return_ty)
            .or_else(|| {
                if owner != self.validator.class_name.as_str() {
                    return None;
                }
                self.validator
                    .methods
                    .get(&Ustr::from(name))
                    .filter(|sig| sig.access_flags & crate::classfile::ACC_STATIC != 0)
                    .map(|sig| sig.return_type.clone())
            })
    }

    fn resolve_current_method(&self, name: Ustr, _args: &[Ty]) -> Option<Ty> {
        self.validator
            .methods
            .get(&name)
            .map(|sig| sig.return_type.clone())
            .or_else(|| {
                self.validator.enclosing_static_owner.and_then(|owner| {
                    self.validator
                        .catalog
                        .resolve_static_method(owner.as_str(), name.as_str(), _args)
                        .map(|method| method.return_ty)
                })
            })
    }

    fn this_ty(&self) -> Ty {
        Ty::Class(self.validator.class_name)
    }

    fn super_ty(&self) -> Ty {
        Ty::Class(self.validator.super_name)
    }
}

fn case_stmts(case: &SwitchCase) -> &[StmtId] {
    match case {
        SwitchCase::Case { body, .. } | SwitchCase::Default { body, .. } => body,
    }
}

fn static_class_name(body: &Body, expr_id: ExprId) -> Option<&str> {
    match &body.exprs[expr_id] {
        Expr::ClassName(name) => Some(name.as_str()),
        _ => None,
    }
}

fn is_current_instance(body: &Body, expr_id: ExprId) -> bool {
    matches!(body.exprs[expr_id], Expr::This | Expr::Super)
}

fn pattern_binding(body: &Body, expr_id: ExprId) -> Option<(Ustr, Ty)> {
    match &body.exprs[expr_id] {
        Expr::Instanceof {
            ty,
            binding: Some(name),
            ..
        } => Some((*name, ty.clone())),
        Expr::Parens(inner) => pattern_binding(body, *inner),
        _ => None,
    }
}
