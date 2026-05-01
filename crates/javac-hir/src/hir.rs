use javac_ty::{Ty, MethodSig, TypeParam};
use std::rc::Rc;
use la_arena::{Arena, Idx};
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

#[derive(Debug, Clone, Default)]
pub struct Body {
    pub exprs: Arena<Expr>,
    pub stmts: Arena<Stmt>,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub id: HirId,
    pub name: Ustr,
    pub ty: Ty,
    pub access_flags: u16,
    pub body: Body,
    pub initializer: Option<ExprId>,
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub id: HirId,
    pub name: Ustr,
    pub signature: MethodSig,
    pub access_flags: u16,
    pub body: Body,
    pub root_block: Option<Block>,
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
    pub resources: Vec<ExprId>,
    pub body: Block,
    pub catches: Vec<CatchClause>,
    pub finally: Option<Block>,
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

    Cast {
        ty: Ty,
        expr: ExprId,
    },

    Instanceof {
        expr: ExprId,
        ty: Ty,
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
    pub name: Ustr,
}

#[derive(Debug, Clone)]
pub enum LambdaBody {
    Expr(ExprId),
    Block(Block),
}
