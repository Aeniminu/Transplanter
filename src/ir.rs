#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub functions: Vec<Function>,
    pub main: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Comment(String),
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
