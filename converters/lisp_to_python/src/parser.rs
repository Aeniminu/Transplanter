use crate::error::LispToPythonError;
use crate::ir::{Expr, ExprKind};
use crate::lexer::{Token, TokenKind};

pub fn parse(tokens: &[Token]) -> Result<Vec<Expr>, LispToPythonError> {
    let mut parser = Parser { tokens, index: 0 };
    let mut forms = Vec::new();

    while !parser.is_at_end() {
        forms.push(parser.parse_expr()?);
    }

    Ok(forms)
}

struct Parser<'a> {
    tokens: &'a [Token],
    index: usize,
}

impl Parser<'_> {
    fn parse_expr(&mut self) -> Result<Expr, LispToPythonError> {
        let Some(token) = self.advance().cloned() else {
            return Err(LispToPythonError::new("式が必要です", 1, 1));
        };

        match &token.kind {
            TokenKind::LParen => self.parse_list(token.line, token.column),
            TokenKind::RParen => Err(LispToPythonError::new(
                "対応する `(` がない `)` です",
                token.line,
                token.column,
            )),
            TokenKind::String(value) => Ok(Expr::new(
                ExprKind::String(value.clone()),
                token.line,
                token.column,
            )),
            TokenKind::Symbol(value) => Ok(Expr::new(atom_kind(value), token.line, token.column)),
        }
    }

    fn parse_list(&mut self, line: usize, column: usize) -> Result<Expr, LispToPythonError> {
        let mut items = Vec::new();

        while let Some(token) = self.peek() {
            if matches!(token.kind, TokenKind::RParen) {
                self.advance();
                return Ok(Expr::new(ExprKind::List(items), line, column));
            }
            items.push(self.parse_expr()?);
        }

        Err(LispToPythonError::new(
            "リストを閉じる `)` が必要です",
            line,
            column,
        ))
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.index)?;
        self.index += 1;
        Some(token)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.tokens.len()
    }
}

fn atom_kind(value: &str) -> ExprKind {
    if value == "#t" || value == "true" {
        return ExprKind::Bool(true);
    }
    if value == "#f" || value == "false" {
        return ExprKind::Bool(false);
    }
    if let Some(keyword) = value.strip_prefix(':') {
        return ExprKind::Keyword(keyword.to_string());
    }
    if is_number_literal(value) {
        return ExprKind::Number(value.to_string());
    }
    ExprKind::Symbol(value.to_string())
}

fn is_number_literal(value: &str) -> bool {
    if value.is_empty() || value == "-" || value == "+" {
        return false;
    }

    let mut chars = value.chars();
    if matches!(chars.clone().next(), Some('-' | '+')) {
        chars.next();
    }

    let mut saw_digit = false;
    let mut saw_dot = false;
    for ch in chars {
        if ch.is_ascii_digit() {
            saw_digit = true;
        } else if ch == '.' && !saw_dot {
            saw_dot = true;
        } else {
            return false;
        }
    }

    saw_digit
}
