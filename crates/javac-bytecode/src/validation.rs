#[path = "validation/diagnostic.rs"]
mod diagnostic;
#[path = "validation/scope.rs"]
mod scope;

use crate::error::BytecodeError;
use diagnostic::{display_internal_name, unresolved_field, unresolved_method, unresolved_variable};
use javac_call_resolver::ClassCatalog;
use javac_hir::hir::*;
use javac_ty::{MethodSig, Ty};
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
    fields: HashMap<Ustr, FieldInfo>,
    methods: HashMap<Ustr, MethodSig>,
}

#[derive(Clone)]
struct FieldInfo {
    ty: Ty,
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

        Self {
            catalog: catalog.clone(),
            class_name: type_decl.name,
            fields,
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
            Expr::Lambda { body: lambda, .. } => match lambda {
                LambdaBody::Expr(expr) => self.validate_expr(body, scope, *expr),
                LambdaBody::Block(block) => self.validate_block(body, &mut scope.clone(), block),
            },
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
        if scope.locals.contains_key(&name) || self.fields.contains_key(&name) {
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
        if let Some(owner) = static_class_name(body, target) {
            if self
                .catalog
                .resolve_static_field(owner, field.as_str())
                .is_none()
            {
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
            let receiver = self.expr_ty(body, scope, target);
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
            return Err(unresolved_method(
                method,
                arg_types,
                &display_internal_name(self.class_name.as_str()),
                line,
            ));
        };
        let is_static = sig.access_flags & javac_classfile::ACC_STATIC != 0;
        if is_static && !allow_static {
            return Err(BytecodeError::new(format!(
                "non-static method {} cannot be called on this",
                method
            )));
        }
        Ok(())
    }

    fn expr_ty(&self, body: &Body, scope: &MethodScope, expr_id: ExprId) -> Ty {
        match &body.exprs[expr_id] {
            Expr::Ident(name) => scope
                .locals
                .get(name)
                .cloned()
                .or_else(|| self.fields.get(name).map(|field| field.ty.clone()))
                .unwrap_or_else(|| self.intrinsic_expr_ty(body, scope, expr_id)),
            Expr::ClassName(name) => Ty::Class(*name),
            Expr::FieldAccess { target, field } => {
                if let Some(owner) = static_class_name(body, *target)
                    && let Some(field_ref) =
                        self.catalog.resolve_static_field(owner, field.as_str())
                {
                    return field_ref.ty;
                }
                if is_current_instance(body, *target)
                    && let Some(field) = self.fields.get(field)
                {
                    return field.ty.clone();
                }
                self.intrinsic_expr_ty(body, scope, expr_id)
            }
            Expr::MethodCall {
                target,
                method,
                args,
            } => {
                let arg_types = args
                    .iter()
                    .map(|arg| self.expr_ty(body, scope, *arg))
                    .collect::<Vec<_>>();
                if let Some(target) = target {
                    let receiver = self.expr_ty(body, scope, *target);
                    if let Some(method_ref) =
                        self.catalog
                            .resolve_instance_method(&receiver, method.as_str(), &arg_types)
                    {
                        return method_ref.return_ty;
                    }
                }
                self.methods
                    .get(method)
                    .map(|sig| sig.return_type.clone())
                    .unwrap_or_else(|| self.intrinsic_expr_ty(body, scope, expr_id))
            }
            Expr::Binary { op, left, right } => match op {
                BinaryOp::AndAnd
                | BinaryOp::OrOr
                | BinaryOp::Eq
                | BinaryOp::Ne
                | BinaryOp::Lt
                | BinaryOp::Gt
                | BinaryOp::Le
                | BinaryOp::Ge => Ty::Boolean,
                BinaryOp::Add
                    if is_string(&self.expr_ty(body, scope, *left))
                        || is_string(&self.expr_ty(body, scope, *right)) =>
                {
                    Ty::string()
                }
                _ => self.expr_ty(body, scope, *left),
            },
            Expr::Unary { op, operand } => match op {
                UnaryOp::Not => Ty::Boolean,
                _ => self.expr_ty(body, scope, *operand),
            },
            Expr::NewArray { element_type, .. } => Ty::Array(Box::new(element_type.clone())),
            Expr::ArrayAccess { array, .. } => match self.expr_ty(body, scope, *array) {
                Ty::Array(element) => *element,
                _ => Ty::Int,
            },
            Expr::Ternary { then_expr, .. } => self.expr_ty(body, scope, *then_expr),
            Expr::Assign { target, .. } => self.expr_ty(body, scope, *target),
            Expr::Parens(inner) => self.expr_ty(body, scope, *inner),
            Expr::Cast { ty, .. } => ty.clone(),
            Expr::Instanceof { .. } => Ty::Boolean,
            Expr::Switch { ty, .. } => ty.clone(),
            _ => self.intrinsic_expr_ty(body, scope, expr_id),
        }
    }

    fn intrinsic_expr_ty(&self, body: &Body, scope: &MethodScope, expr_id: ExprId) -> Ty {
        match &body.exprs[expr_id] {
            Expr::IntLiteral(_) => Ty::Int,
            Expr::LongLiteral(_) => Ty::Long,
            Expr::FloatLiteral(_) => Ty::Float,
            Expr::DoubleLiteral(_) => Ty::Double,
            Expr::BoolLiteral(_) => Ty::Boolean,
            Expr::CharLiteral(_) => Ty::Char,
            Expr::StringLiteral(_) => Ty::string(),
            Expr::NullLiteral | Expr::This | Expr::Super => Ty::object(),
            Expr::ClassName(name) => Ty::Class(*name),
            Expr::NewObject { class, .. } => class.clone(),
            Expr::Lambda { .. } | Expr::MethodRef { .. } => Ty::object(),
            Expr::PostInc(inner) | Expr::PostDec(inner) | Expr::Parens(inner) => {
                self.expr_ty(body, scope, *inner)
            }
            Expr::Assign { value, .. } => self.expr_ty(body, scope, *value),
            _ => Ty::Int,
        }
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
    matches!(body.exprs[expr_id], Expr::This)
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

fn is_string(ty: &Ty) -> bool {
    ty.is_string()
}
