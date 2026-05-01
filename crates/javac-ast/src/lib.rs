use rowan::{Language, SyntaxKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum JavaSyntaxKind {
    AbstractKw,
    AssertKw,
    BooleanKw,
    BreakKw,
    ByteKw,
    CaseKw,
    CatchKw,
    CharKw,
    ClassKw,
    ContinueKw,
    DefaultKw,
    DoKw,
    DoubleKw,
    ElseKw,
    EnumKw,
    ExtendsKw,
    FinalKw,
    FinallyKw,
    FloatKw,
    ForKw,
    IfKw,
    ImplementsKw,
    ImportKw,
    InstanceofKw,
    IntKw,
    InterfaceKw,
    LongKw,
    NativeKw,
    NewKw,
    PackageKw,
    PrivateKw,
    ProtectedKw,
    PublicKw,
    ReturnKw,
    ShortKw,
    StaticKw,
    StrictfpKw,
    SuperKw,
    SwitchKw,
    SynchronizedKw,
    ThisKw,
    ThrowKw,
    ThrowsKw,
    TransientKw,
    TryKw,
    VoidKw,
    VolatileKw,
    WhileKw,
    YieldKw,
    RecordKw,
    SealedKw,
    NonSealedKw,
    PermitsKw,
    VarKw,

    IntLiteral,
    LongLiteral,
    FloatLiteral,
    DoubleLiteral,
    CharLiteral,
    StringLiteral,
    TextBlockLiteral,
    TrueKw,
    FalseKw,
    NullKw,

    Ident,

    LBrace,
    RBrace,
    LBrack,
    RBrack,
    LParen,
    RParen,
    Semi,
    Comma,
    Dot,
    Ellipsis,
    At,
    ColonColon,
    Arrow,

    Eq,
    Gt,
    Lt,
    Bang,
    Tilde,
    Question,
    Colon,

    EqEq,
    Le,
    Ge,
    Neq,

    Inc,
    Dec,

    AmpAmp,
    PipePipe,

    Plus,
    Minus,
    Star,
    Slash,
    Amp,
    Pipe,
    Caret,
    Percent,

    LtLt,
    GtGt,
    GtGtGt,

    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    AmpEq,
    PipeEq,
    CaretEq,
    PercentEq,
    LtLtEq,
    GtGtEq,
    GtGtGtEq,

    Underscore,

    CompilationUnit,
    PackageDecl,
    ImportDecl,
    ImportWildcard,
    ClassDecl,
    InterfaceDecl,
    EnumDecl,
    RecordDecl,
    AnnotationDecl,
    ModifierList,

    SealedModifier,
    NonSealedModifier,
    PermitsClause,

    TypeParamList,
    TypeParam,
    TypeBound,

    ExtendsClause,
    ImplementsClause,

    ClassBody,
    ClassMemberList,
    ClassMember,

    MethodDecl,
    ConstructorDecl,
    FieldDecl,
    StaticInit,
    InstanceInit,

    FormalParamList,
    FormalParam,
    VarargsParam,
    ReceiverParam,

    ThrowsClause,
    ExceptionTypeList,

    MethodBody,

    Block,
    StmtList,

    ExprStmt,
    EmptyStmt,
    LocalVarDecl,
    VarDeclarator,
    VarDeclaratorList,

    IfStmt,
    ForStmt,
    WhileStmt,
    DoStmt,
    SwitchStmt,
    SwitchBlock,
    SwitchRule,
    SwitchLabel,
    YieldStmt,
    SynchronizedStmt,
    TryStmt,
    CatchClause,
    FinallyClause,
    TryWithResources,
    ResourceList,
    Resource,
    ReturnStmt,
    ThrowStmt,
    AssertStmt,
    BreakStmt,
    ContinueStmt,
    LabeledStmt,

    ForInit,
    ForCond,
    ForUpdate,
    ForEach,

    Type,
    PrimitiveType,
    ClassType,
    ClassTypeSegment,
    ArrayType,
    IntersectionType,
    UnionType,
    TypeArgList,
    WildcardType,

    Name,
    QualifiedName,
    MemberSelect,

    BinaryExpr,
    UnaryExpr,
    PostfixExpr,
    ParenExpr,
    ConditionalExpr,
    AssignExpr,
    CompoundAssignExpr,

    Literal,

    MethodInvocation,
    NewExpr,
    ArrayAccess,
    FieldAccess,
    ThisExpr,
    SuperExpr,
    NullLiteralExpr,
    ArrayInit,
    Dim,

    CastExpr,
    InstanceofExpr,
    PatternExpr,

    LambdaExpr,
    MethodRefExpr,

    Annotation,
    AnnotationArgList,
    AnnotationArg,
    AnnotationElemDecl,

    RecordComponent,
    RecordComponentList,

    EnumConstant,
    EnumConstantList,
    EnumBody,

    TextBlock,

    PatternMatch,
    GuardPattern,
    DeconstructionPattern,
    DeconstructionArgList,

    Comment,
    Whitespace,
    Error,
}

impl From<JavaSyntaxKind> for SyntaxKind {
    fn from(kind: JavaSyntaxKind) -> Self {
        Self(kind as u16)
    }
}

impl From<SyntaxKind> for JavaSyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        assert!(kind.0 <= Self::Error as u16);
        unsafe { std::mem::transmute::<u16, JavaSyntaxKind>(kind.0) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JavaLanguage;

impl Language for JavaLanguage {
    type Kind = JavaSyntaxKind;

    fn kind_from_raw(raw: SyntaxKind) -> Self::Kind {
        Self::Kind::from(raw)
    }

    fn kind_to_raw(kind: Self::Kind) -> SyntaxKind {
        kind.into()
    }
}

pub type JavaSyntaxNode = rowan::SyntaxNode<JavaLanguage>;
pub type JavaSyntaxToken = rowan::SyntaxToken<JavaLanguage>;
pub type JavaSyntaxElement = rowan::SyntaxElement<JavaLanguage>;