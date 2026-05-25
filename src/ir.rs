#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub constants: Vec<Constant>,
    pub struct_factories: Vec<StructFactory>,
    pub namespace_aliases: Vec<NamespaceAlias>,
    pub functions: Vec<Function>,
    pub main: Vec<Stmt>,
}

pub type FarmIr = Program;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constant {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructFactory {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceAlias {
    pub path: Vec<String>,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<FunctionParam>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionParam {
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Noop,
    Comment(String),
    Function(Function),
    Loop(Vec<Stmt>),
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_branch: Option<ElseBranch>,
    },
    For {
        variable: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
    },
    ForEach {
        variable: String,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    Let {
        name: String,
        value: Expr,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Expr(Expr),
    Break,
    Continue,
    Return(Option<Expr>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElseBranch {
    ElseIf {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_branch: Option<Box<ElseBranch>>,
    },
    Else(Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    pub tokens: Vec<ExprToken>,
}

impl Expr {
    pub fn new(tokens: Vec<ExprToken>) -> Self {
        Self { tokens }
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprToken {
    Ident(String),
    Number(String),
    String(String),
    Symbol(char),
    Operator(String),
}
