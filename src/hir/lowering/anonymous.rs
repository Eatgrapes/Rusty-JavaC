use crate::ast::{AstNode, ClassDecl, JavaSyntaxKind, JavaSyntaxNode};
use crate::hir::lowering::expr::BodyBuilder;
use crate::hir::lowering::member::lower_class_members;
use crate::hir::lowering::syntax::ExprToken;
use crate::hir::lowering::{LowerError, LowerResult};
use crate::hir::*;
use crate::ty::{MethodSig, Ty};
use std::rc::Rc;
use ustr::Ustr;

pub(super) fn lower_object(
    body: &mut BodyBuilder,
    base_type: Ty,
    args: Vec<ExprId>,
    body_tokens: Vec<ExprToken>,
) -> LowerResult<ExprId> {
    let class_body = parse_class_body(&body_tokens)?;
    let class_name = next_class_name(body, &body_tokens)?;
    let base_internal_name = base_type.internal_name();
    let is_interface = body.type_resolver.is_interface(&base_internal_name);
    let super_name = if is_interface {
        "java/lang/Object".to_string()
    } else {
        base_internal_name
    };
    let arg_types = args
        .iter()
        .map(|arg| body.expr_ty(*arg))
        .collect::<Vec<_>>();
    let super_params = if is_interface {
        Vec::new()
    } else {
        body.type_resolver
            .resolve_constructor(&base_type, &arg_types)
            .map(|method| method.params)
            .unwrap_or_else(|| arg_types.clone())
    };
    let captures_this = body.can_capture_this;
    let outer_this = captures_this.then(|| OuterThisInfo {
        field_name: Ustr::from("this$0"),
        ty: body.type_resolver.current_class_ty(),
    });
    let super_call = SuperConstructorCall {
        owner: if is_interface {
            Ty::object()
        } else {
            base_type.clone()
        },
        params: super_params.clone(),
        arg_offset: usize::from(captures_this),
    };
    let mut members = lower_class_members(
        class_body,
        &[],
        &body
            .type_resolver
            .for_anonymous_class(class_name.as_str(), &super_name),
        body.enclosing_static_owner,
    )?;

    let mut fields = Vec::new();
    if let Some(outer_this) = &outer_this {
        fields.push(outer_this_field(outer_this));
    }
    fields.append(&mut members.fields);

    let mut methods = vec![anonymous_constructor(
        anonymous_constructor_params(outer_this.as_ref(), &super_params),
        super_call.clone(),
    )];
    methods.append(&mut members.methods);

    body.anonymous_types.push(Rc::new(TypeDecl {
        id: HirId(0),
        name: class_name,
        kind: TypeDeclKind::Class,
        access_flags: 0,
        super_class: (!is_interface).then_some(base_type.clone()),
        interfaces: is_interface
            .then_some(base_type.clone())
            .into_iter()
            .collect(),
        type_params: Vec::new(),
        generic_signature: None,
        fields,
        methods,
        inner_types: members.inner_types,
        anonymous: Some(AnonymousClassInfo {
            super_constructor: super_call,
            outer_this,
            enclosing_static_owner: body.enclosing_static_owner,
            outer_fields: body.outer_fields.clone(),
        }),
        record_components: Vec::new(),
        annotations: Vec::new(),
    }));

    Ok(body.alloc_expr(Expr::NewObject {
        class: base_type,
        args,
        anonymous: Some(AnonymousObject {
            class_name,
            constructor_params: super_params,
            captures_enclosing_this: captures_this,
        }),
    }))
}

fn next_class_name(body: &mut BodyBuilder, tokens: &[ExprToken]) -> LowerResult<Ustr> {
    let owner = body
        .type_resolver
        .current_class_name()
        .ok_or_else(|| unsupported_anonymous_class(tokens))?;
    let next = body.anonymous_counter.get() + 1;
    body.anonymous_counter.set(next);
    Ok(Ustr::from(&format!("{owner}${next}")))
}

fn parse_class_body(tokens: &[ExprToken]) -> LowerResult<crate::ast::ClassBody> {
    let body_source = token_source(tokens);
    let source = if tokens
        .first()
        .is_some_and(|token| token.kind == JavaSyntaxKind::LBrace)
    {
        format!("class Anonymous {body_source}")
    } else {
        format!("class Anonymous {{ {body_source} }}")
    };
    let parse = crate::parser::Parser::parse(&source);
    if !parse.errors.is_empty() {
        return Err(unsupported_anonymous_class(tokens));
    }

    let root = JavaSyntaxNode::new_root(parse.green_node);
    ClassDecl::cast(root.clone())
        .or_else(|| root.children().find_map(ClassDecl::cast))
        .and_then(|class| class.body())
        .ok_or_else(|| unsupported_anonymous_class(tokens))
}

fn unsupported_anonymous_class(tokens: &[ExprToken]) -> LowerError {
    tokens
        .first()
        .map(|token| LowerError::UnsupportedExpressionAt {
            line: token.line,
            range: Some(token.range),
        })
        .unwrap_or(LowerError::UnsupportedExpression)
}

fn token_source(tokens: &[ExprToken]) -> String {
    let mut source = String::new();
    let mut line = 1u16;
    for token in tokens {
        while line < token.line {
            source.push('\n');
            line += 1;
        }
        if !source.ends_with(['\n', ' ', '{', '(']) {
            source.push(' ');
        }
        source.push_str(&token.text);
    }
    source
}

fn anonymous_constructor_params(
    outer_this: Option<&OuterThisInfo>,
    super_params: &[Ty],
) -> Vec<ParamDecl> {
    let mut params = Vec::new();
    if let Some(outer_this) = outer_this {
        params.push(ParamDecl {
            name: outer_this.field_name,
            ty: outer_this.ty.clone(),
        });
    }
    params.extend(
        super_params
            .iter()
            .enumerate()
            .map(|(index, ty)| ParamDecl {
                name: Ustr::from(&format!("arg{index}")),
                ty: ty.clone(),
            }),
    );
    params
}

fn anonymous_constructor(
    params: Vec<ParamDecl>,
    constructor_call: SuperConstructorCall,
) -> MethodDecl {
    let signature = MethodSig::new(
        Ustr::from("<init>"),
        params.iter().map(|param| param.ty.clone()).collect(),
        Ty::Void,
    );
    MethodDecl {
        id: HirId(0),
        name: Ustr::from("<init>"),
        params,
        signature,
        access_flags: 0,
        source_line: None,
        generic_signature: None,
        throws: Vec::new(),
        body: Body::default(),
        root_block: Some(Block { stmts: Vec::new() }),
        constructor_call: Some(constructor_call),
    }
}

fn outer_this_field(outer_this: &OuterThisInfo) -> FieldDecl {
    FieldDecl {
        id: HirId(0),
        name: outer_this.field_name,
        ty: outer_this.ty.clone(),
        access_flags: crate::classfile::ACC_PRIVATE
            | crate::classfile::ACC_FINAL
            | crate::classfile::ACC_SYNTHETIC,
        generic_signature: None,
        body: Body::default(),
        initializer: None,
    }
}
