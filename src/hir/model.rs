use crate::ty::{MethodSig, Ty, TypeParam};
use la_arena::{Arena, Idx};
use std::collections::HashMap;
use std::rc::Rc;
use text_size::TextRange;
use ustr::Ustr;

pub type ExprId = Idx<Expr>;
pub type StmtId = Idx<Stmt>;

#[derive(Debug, Clone, PartialEq)]
pub struct HirId(pub u32);

#[derive(Debug, Clone)]
pub struct Package {
    pub name: Ustr,
}

#[derive(Debug, Clone)]
pub struct CompilationUnit {
    pub package: Option<Package>,
    pub imports: Vec<Import>,
    pub type_decls: Vec<TypeDecl>,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Ustr,
    pub is_static: bool,
    pub is_wildcard: bool,
    pub source_line: Option<u16>,
    pub source_range: Option<TextRange>,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub id: HirId,
    pub name: Ustr,
    pub kind: TypeDeclKind,
    pub access_flags: u16,
    pub super_class: Option<Ty>,
    pub interfaces: Vec<Ty>,
    pub type_params: Vec<TypeParam>,
    pub generic_signature: Option<String>,
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<MethodDecl>,
    pub inner_types: Vec<Rc<TypeDecl>>,
    pub anonymous: Option<AnonymousClassInfo>,
    pub record_components: Vec<RecordComponentDecl>,
    pub annotations: Vec<AnnotationUse>,
}

#[derive(Debug, Clone)]
pub struct AnonymousClassInfo {
    pub super_constructor: SuperConstructorCall,
    pub outer_this: Option<OuterThisInfo>,
    pub enclosing_static_owner: Option<Ustr>,
    pub outer_fields: Vec<CapturedField>,
}

#[derive(Debug, Clone)]
pub struct SuperConstructorCall {
    pub owner: Ty,
    pub params: Vec<Ty>,
    pub arg_offset: usize,
}

#[derive(Debug, Clone)]
pub struct OuterThisInfo {
    pub field_name: Ustr,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub struct CapturedField {
    pub name: Ustr,
    pub ty: Ty,
    pub access_flags: u16,
}

#[derive(Debug, Clone)]
pub struct AnonymousObject {
    pub class_name: Ustr,
    pub constructor_params: Vec<Ty>,
    pub captures_enclosing_this: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeDeclKind {
    Class,
    Interface,
    Enum,
    Record,
    Annotation,
}

#[derive(Debug, Clone, Default)]
pub struct Body {
    pub exprs: Arena<Expr>,
    pub stmts: Arena<Stmt>,
    pub stmt_lines: HashMap<StmtId, u16>,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub id: HirId,
    pub name: Ustr,
    pub ty: Ty,
    pub access_flags: u16,
    pub generic_signature: Option<String>,
    pub body: Body,
    pub initializer: Option<ExprId>,
}

#[derive(Debug, Clone)]
pub struct RecordComponentDecl {
    pub name: Ustr,
    pub ty: Ty,
    pub generic_signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnnotationUse {
    pub descriptor: String,
    pub elements: Vec<AnnotationElement>,
}

#[derive(Debug, Clone)]
pub struct AnnotationElement {
    pub name: Ustr,
    pub value: AnnotationValue,
}

#[derive(Debug, Clone)]
pub enum AnnotationValue {
    String(Ustr),
    Int(i64),
    Boolean(bool),
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub id: HirId,
    pub name: Ustr,
    pub params: Vec<ParamDecl>,
    pub signature: MethodSig,
    pub access_flags: u16,
    pub source_line: Option<u16>,
    pub generic_signature: Option<String>,
    pub throws: Vec<Ty>,
    pub body: Body,
    pub root_block: Option<Block>,
    pub constructor_call: Option<SuperConstructorCall>,
}

#[derive(Debug, Clone)]
pub struct ParamDecl {
    pub name: Ustr,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<StmtId>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(ExprId),
    Empty,
    LocalVar(LocalVarDecl),
    If {
        condition: ExprId,
        then_branch: StmtId,
        else_branch: Option<StmtId>,
    },
    For {
        init: Option<StmtId>,
        condition: Option<ExprId>,
        update: Option<ExprId>,
        body: StmtId,
    },
    ForEach {
        var_type: Ty,
        var_name: Ustr,
        iterable: ExprId,
        body: StmtId,
    },
    While {
        condition: ExprId,
        body: StmtId,
    },
    Do {
        body: StmtId,
        condition: ExprId,
    },
    Return(Option<ExprId>),
    Throw(ExprId),
    Break(Option<Ustr>),
    Continue(Option<Ustr>),
    Labeled {
        label: Ustr,
        body: StmtId,
    },
    Switch {
        selector: ExprId,
        cases: Vec<SwitchCase>,
    },
    Try(TryStmt),
    Synchronized(ExprId, Block),
    Assert {
        condition: ExprId,
        message: Option<ExprId>,
    },
    Yield(ExprId),
    Block(Block),
}

#[derive(Debug, Clone)]
pub struct LocalVarDecl {
    pub ty: Ty,
    pub name: Ustr,
    pub initializer: Option<ExprId>,
}

#[derive(Debug, Clone)]
pub enum SwitchCase {
    Case {
        pattern: ExprId,
        body: Vec<StmtId>,
        is_arrow: bool,
    },
    Default {
        body: Vec<StmtId>,
        is_arrow: bool,
    },
}

#[derive(Debug, Clone)]
pub struct TryStmt {
    pub resources: Vec<TryResource>,
    pub body: Block,
    pub catches: Vec<CatchClause>,
    pub finally: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct TryResource {
    pub ty: Ty,
    pub name: Ustr,
    pub initializer: Option<ExprId>,
}

#[derive(Debug, Clone)]
pub struct CatchClause {
    pub exception_type: Ty,
    pub var_name: Ustr,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLiteral(i64),
    LongLiteral(i64),
    FloatLiteral(f32),
    DoubleLiteral(f64),
    BoolLiteral(bool),
    CharLiteral(char),
    StringLiteral(Ustr),
    NullLiteral,
    This,
    Super,

    Ident(Ustr),
    ClassName(Ustr),

    FieldAccess {
        target: ExprId,
        field: Ustr,
    },

    MethodCall {
        target: Option<ExprId>,
        method: Ustr,
        args: Vec<ExprId>,
    },

    NewObject {
        class: Ty,
        args: Vec<ExprId>,
        anonymous: Option<AnonymousObject>,
    },

    NewArray {
        element_type: Ty,
        dimensions: Vec<Option<ExprId>>,
        initializer: Option<ArrayInit>,
    },

    ArrayAccess {
        array: ExprId,
        index: ExprId,
    },

    Unary {
        op: UnaryOp,
        operand: ExprId,
    },

    Binary {
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
    },

    Ternary {
        condition: ExprId,
        then_expr: ExprId,
        else_expr: ExprId,
    },

    Switch {
        selector: ExprId,
        cases: Vec<SwitchCase>,
        ty: Ty,
    },

    Cast {
        ty: Ty,
        expr: ExprId,
    },

    Instanceof {
        expr: ExprId,
        ty: Ty,
        binding: Option<Ustr>,
    },

    Assign {
        target: ExprId,
        op: AssignOp,
        value: ExprId,
    },

    PostInc(ExprId),
    PostDec(ExprId),

    Lambda {
        params: Vec<LambdaParam>,
        body: LambdaBody,
        target_ty: Option<Ty>,
    },

    MethodRef {
        target: ExprId,
        method: Ustr,
    },

    Parens(ExprId),
}

#[derive(Debug, Clone)]
pub struct ArrayInit {
    pub elements: Vec<ExprId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    PreInc,
    PreDec,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    Ushr,
    And,
    Or,
    Xor,
    AndAnd,
    OrOr,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignOp {
    Plain,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    Ushr,
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone)]
pub struct LambdaParam {
    pub ty: Option<Ty>,
    pub name: Ustr,
}

#[derive(Debug, Clone)]
pub enum LambdaBody {
    Expr(ExprId),
    Block(Block),
}
