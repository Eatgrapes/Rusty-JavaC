use javac_ty::{Ty, MethodSig, TypeParam};
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct HirId(pub u32);

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct CompilationUnit {
    pub package: Option<Package>,
    pub imports: Vec<Import>,
    pub type_decls: Vec<TypeDecl>,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: String,
    pub is_static: bool,
    pub is_wildcard: bool,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub id: HirId,
    pub name: String,
    pub kind: TypeDeclKind,
    pub access_flags: u16,
    pub super_class: Option<Ty>,
    pub interfaces: Vec<Ty>,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<MethodDecl>,
    pub inner_types: Vec<Rc<TypeDecl>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeDeclKind {
    Class,
    Interface,
    Enum,
    Record,
    Annotation,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub id: HirId,
    pub name: String,
    pub ty: Ty,
    pub access_flags: u16,
    pub initializer: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub id: HirId,
    pub name: String,
    pub signature: MethodSig,
    pub body: Option<Block>,
    pub access_flags: u16,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Empty,
    LocalVar(LocalVarDecl),
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    ForEach {
        var_type: Ty,
        var_name: String,
        iterable: Expr,
        body: Box<Stmt>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    Do {
        body: Box<Stmt>,
        condition: Expr,
    },
    Return(Option<Expr>),
    Throw(Expr),
    Break(Option<String>),
    Continue(Option<String>),
    Try(TryStmt),
    Synchronized(Expr, Block),
    Assert {
        condition: Expr,
        message: Option<Expr>,
    },
    Yield(Expr),
    Block(Block),
}

#[derive(Debug, Clone)]
pub struct LocalVarDecl {
    pub ty: Ty,
    pub name: String,
    pub initializer: Option<Expr>,
}

#[derive(Debug, Clone)]
pub enum SwitchCase {
    Case {
        pattern: Expr,
        body: Vec<Stmt>,
        is_arrow: bool,
    },
    Default {
        body: Vec<Stmt>,
        is_arrow: bool,
    },
}

#[derive(Debug, Clone)]
pub struct TryStmt {
    pub resources: Vec<Expr>,
    pub body: Block,
    pub catches: Vec<CatchClause>,
    pub finally: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct CatchClause {
    pub exception_type: Ty,
    pub var_name: String,
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
    StringLiteral(String),
    NullLiteral,
    This,
    Super,

    Ident(String),

    FieldAccess {
        target: Box<Expr>,
        field: String,
    },

    MethodCall {
        target: Option<Box<Expr>>,
        method: String,
        args: Vec<Expr>,
    },

    NewObject {
        class: Ty,
        args: Vec<Expr>,
    },

    NewArray {
        element_type: Ty,
        dimensions: Vec<Option<Expr>>,
        initializer: Option<ArrayInit>,
    },

    ArrayAccess {
        array: Box<Expr>,
        index: Box<Expr>,
    },

    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },

    Cast {
        ty: Ty,
        expr: Box<Expr>,
    },

    Instanceof {
        expr: Box<Expr>,
        ty: Ty,
    },

    Assign {
        target: Box<Expr>,
        op: AssignOp,
        value: Box<Expr>,
    },

    PostInc(Box<Expr>),
    PostDec(Box<Expr>),

    Lambda {
        params: Vec<LambdaParam>,
        body: Box<LambdaBody>,
    },

    MethodRef {
        target: Box<Expr>,
        method: String,
    },

    Parens(Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct ArrayInit {
    pub elements: Vec<Expr>,
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
    Add, Sub, Mul, Div, Rem,
    Shl, Shr, Ushr,
    And, Or, Xor,
    AndAnd, OrOr,
    Eq, Ne, Lt, Gt, Le, Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignOp {
    Plain, Add, Sub, Mul, Div, Rem,
    Shl, Shr, Ushr, And, Or, Xor,
}

#[derive(Debug, Clone)]
pub struct LambdaParam {
    pub ty: Option<Ty>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Block),
}