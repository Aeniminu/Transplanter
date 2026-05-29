#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub line: usize,
    pub column: usize,
}

impl Expr {
    pub fn new(kind: ExprKind, line: usize, column: usize) -> Self {
        Self { kind, line, column }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Bool(bool),
    Keyword(String),
    List(Vec<Expr>),
    Number(String),
    String(String),
    Symbol(String),
}
