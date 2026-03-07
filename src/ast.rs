use crate::diagnostic::Position;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Import { name: Name, position: Position },
    Use { name: Name, position: Position },
    Subprogram(Subprogram),
    Type(TypeDecl),
    Package(Package),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name {
    pub segments: Vec<String>,
}

impl Name {
    pub fn as_string(&self) -> String {
        self.segments.join(".")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub is_body: bool,
    pub name: Name,
    pub items: Vec<PackageItem>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageItem {
    Subprogram(Subprogram),
    Type(TypeDecl),
    Object(LocalDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subprogram {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Name>,
    pub requires: Vec<Expr>,
    pub ensures: Vec<Expr>,
    pub global: Option<GlobalContract>,
    pub depends: Option<DependsContract>,
    pub body: Option<Block>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalContract {
    pub items: Vec<GlobalItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalItem {
    pub mode: GlobalMode,
    pub names: Vec<Name>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlobalMode {
    Null,
    Input,
    Output,
    InOut,
    ProofIn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependsContract {
    pub items: Vec<DependItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependItem {
    pub target: DependTarget,
    pub sources: Vec<Name>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DependTarget {
    Null,
    Result,
    Name(Name),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub mode: ParamMode,
    pub ty: Name,
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArg {
    pub name: Option<String>,
    pub value: Expr,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDecl {
    Record(RecordType),
    Enum(EnumType),
    Range(RangeType),
    Array(ArrayType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordType {
    pub name: String,
    pub fields: Vec<RecordField>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordField {
    pub ty: Name,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFieldInit {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
    pub name: String,
    pub variants: Vec<String>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeType {
    pub name: String,
    pub base: Name,
    pub start: Expr,
    pub end: Expr,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayType {
    pub name: String,
    pub start: Expr,
    pub end: Expr,
    pub element_type: Name,
    pub position: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamMode {
    In,
    Out,
    InOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub items: Vec<BlockItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockItem {
    LocalDecl(LocalDecl),
    Statement(Statement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDecl {
    pub is_const: bool,
    pub ty: Name,
    pub name: String,
    pub initializer: Option<Expr>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Null {
        position: Position,
    },
    Break {
        position: Position,
    },
    Continue {
        position: Position,
    },
    Assert {
        expr: Expr,
        position: Position,
    },
    Return {
        expr: Option<Expr>,
        position: Position,
    },
    Assign {
        target: Expr,
        value: Expr,
        position: Position,
    },
    Expr {
        expr: Expr,
        position: Position,
    },
    If(IfStatement),
    Case(CaseStatement),
    While {
        condition: Expr,
        invariants: Vec<Expr>,
        variants: Vec<LoopVariant>,
        body: StatementBlock,
        position: Position,
    },
    For(ForStatement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementBlock {
    pub items: Vec<BlockItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStatement {
    pub condition: Expr,
    pub then_branch: StatementBlock,
    pub else_if_branches: Vec<ElseIfBranch>,
    pub else_branch: Option<StatementBlock>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseStatement {
    pub expr: Expr,
    pub arms: Vec<CaseArm>,
    pub else_arm: Option<StatementBlock>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseArm {
    pub choices: Vec<Expr>,
    pub body: StatementBlock,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElseIfBranch {
    pub condition: Expr,
    pub body: StatementBlock,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForStatement {
    pub iterator_type: Name,
    pub iterator: String,
    pub start: Expr,
    pub end: Expr,
    pub invariants: Vec<Expr>,
    pub variants: Vec<LoopVariant>,
    pub body: StatementBlock,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopVariant {
    pub direction: LoopVariantDirection,
    pub expr: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopVariantDirection {
    Increases,
    Decreases,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Bool {
        value: bool,
        position: Position,
    },
    Integer {
        value: String,
        position: Position,
    },
    Float {
        value: String,
        position: Position,
    },
    Character {
        value: char,
        position: Position,
    },
    String {
        value: String,
        position: Position,
    },
    Name {
        name: Name,
        position: Position,
    },
    Member {
        base: Box<Expr>,
        member: String,
        position: Position,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
        position: Position,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
        position: Position,
    },
    RecordLiteral {
        ty: Name,
        fields: Vec<RecordFieldInit>,
        position: Position,
    },
    ArrayLiteral {
        elements: Vec<Expr>,
        position: Position,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        position: Position,
    },
    Binary {
        lhs: Box<Expr>,
        op: BinaryOp,
        rhs: Box<Expr>,
        position: Position,
    },
}

impl Expr {
    pub fn position(&self) -> Position {
        match self {
            Self::Bool { position, .. }
            | Self::Integer { position, .. }
            | Self::Float { position, .. }
            | Self::Character { position, .. }
            | Self::String { position, .. }
            | Self::Name { position, .. }
            | Self::Member { position, .. }
            | Self::Index { position, .. }
            | Self::Call { position, .. }
            | Self::RecordLiteral { position, .. }
            | Self::ArrayLiteral { position, .. }
            | Self::Unary { position, .. }
            | Self::Binary { position, .. } => *position,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Or,
    ShortCircuitOr,
    And,
    ShortCircuitAnd,
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}
