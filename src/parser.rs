use crate::error::FarmError;
use crate::ir::{ElseBranch, Expr, Function, Program, Stmt};
use crate::lexer::{Token, TokenKind};

pub fn parse(tokens: &[Token]) -> Result<Program, FarmError> {
    Parser { tokens, pos: 0 }.parse_program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn parse_program(&mut self) -> Result<Program, FarmError> {
        let mut functions = Vec::new();
        let mut main = None;

        while !self.is_eof() {
            self.reject_unsupported_starter()?;
            self.expect_keyword("fn")?;
            let function = self.parse_function_after_fn()?;

            if function.name == "main" {
                if !function.params.is_empty() {
                    let token = self.previous();
                    return Err(FarmError::new(
                        "`fn main` must not have parameters",
                        token.line,
                        token.column,
                    ));
                }
                if main.is_some() {
                    let token = self.previous();
                    return Err(FarmError::new(
                        "duplicate `fn main`",
                        token.line,
                        token.column,
                    ));
                }
                main = Some(function.body);
            } else {
                functions.push(function);
            }
        }

        let Some(main) = main else {
            let (line, column) = self.end_position();
            return Err(FarmError::new("missing `fn main`", line, column));
        };

        Ok(Program { functions, main })
    }

    fn parse_function_after_fn(&mut self) -> Result<Function, FarmError> {
        let name = self.expect_ident("expected function name")?;

        if self.check_operator("<") {
            let token = self.peek().expect("checked token exists");
            return Err(FarmError::unsupported("generics", token.line, token.column));
        }

        self.expect_symbol('(')?;
        let params = self.parse_params()?;
        self.expect_symbol(')')?;

        if self.match_operator("->") {
            let token = self.previous();
            return Err(FarmError::unsupported(
                "function return types",
                token.line,
                token.column,
            ));
        }

        let body = self.parse_block()?;
        Ok(Function { name, params, body })
    }

    fn parse_params(&mut self) -> Result<Vec<String>, FarmError> {
        let mut params = Vec::new();

        while !self.check_symbol(')') {
            let param = self.expect_ident("expected parameter name")?;
            params.push(param);

            while !self.check_symbol(')') && !self.check_symbol(',') {
                self.advance();
            }

            if !self.match_symbol(',') {
                break;
            }
        }

        Ok(params)
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, FarmError> {
        self.expect_symbol('{')?;
        let mut body = Vec::new();

        while !self.check_symbol('}') {
            if self.is_eof() {
                let (line, column) = self.end_position();
                return Err(FarmError::new("unterminated block", line, column));
            }
            body.push(self.parse_stmt()?);
        }

        self.expect_symbol('}')?;
        Ok(body)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, FarmError> {
        self.reject_unsupported_starter()?;

        if let Some(comment) = self.match_comment() {
            return Ok(Stmt::Comment(comment));
        }

        if self.match_keyword("loop") {
            return Ok(Stmt::Loop(self.parse_block()?));
        }

        if self.match_keyword("while") {
            let condition =
                self.collect_expr_until_symbol('{', "expected `{` after while condition")?;
            self.require_expr(&condition, "expected while condition")?;
            let body = self.parse_block()?;
            return Ok(Stmt::While { condition, body });
        }

        if self.match_keyword("if") {
            return self.parse_if_after_keyword();
        }

        if self.match_keyword("for") {
            return self.parse_for_after_keyword();
        }

        if self.match_keyword("let") {
            return self.parse_let_after_keyword();
        }

        if self.match_keyword("break") {
            self.expect_semicolon("expected `;` after break")?;
            return Ok(Stmt::Break);
        }

        if self.match_keyword("continue") {
            self.expect_semicolon("expected `;` after continue")?;
            return Ok(Stmt::Continue);
        }

        if self.match_keyword("return") {
            if self.match_symbol(';') {
                return Ok(Stmt::Return(None));
            }
            let value = self.collect_expr_until_semicolon("expected `;` after return value")?;
            return Ok(Stmt::Return(Some(value)));
        }

        self.parse_expr_or_assignment_stmt()
    }

    fn parse_if_after_keyword(&mut self) -> Result<Stmt, FarmError> {
        let condition = self.collect_expr_until_symbol('{', "expected `{` after if condition")?;
        self.require_expr(&condition, "expected if condition")?;
        let then_body = self.parse_block()?;
        let else_branch = self.parse_else_branch()?;
        Ok(Stmt::If {
            condition,
            then_body,
            else_branch,
        })
    }

    fn parse_else_branch(&mut self) -> Result<Option<ElseBranch>, FarmError> {
        if !self.match_keyword("else") {
            return Ok(None);
        }

        if self.match_keyword("if") {
            let condition =
                self.collect_expr_until_symbol('{', "expected `{` after else if condition")?;
            self.require_expr(&condition, "expected else if condition")?;
            let then_body = self.parse_block()?;
            let else_branch = self.parse_else_branch()?.map(Box::new);
            return Ok(Some(ElseBranch::ElseIf {
                condition,
                then_body,
                else_branch,
            }));
        }

        Ok(Some(ElseBranch::Else(self.parse_block()?)))
    }

    fn parse_for_after_keyword(&mut self) -> Result<Stmt, FarmError> {
        let variable = self.expect_ident("expected loop variable")?;
        self.expect_keyword("in")?;

        let start = self.collect_expr_until_operator("..", "expected `..` in for range")?;
        self.require_expr(&start, "expected for range start")?;
        self.expect_operator("..")?;
        let end = self.collect_expr_until_symbol('{', "expected `{` after for range")?;
        self.require_expr(&end, "expected for range end")?;
        let body = self.parse_block()?;

        Ok(Stmt::For {
            variable,
            start,
            end,
            body,
        })
    }

    fn parse_let_after_keyword(&mut self) -> Result<Stmt, FarmError> {
        self.match_keyword("mut");
        let name = self.expect_ident("expected variable name after let")?;

        if !self.match_operator("=") {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            return Err(FarmError::new(
                "expected `=` in let statement",
                line,
                column,
            ));
        }

        let value = self.collect_expr_until_semicolon("expected `;` after let statement")?;
        self.require_expr(&value, "expected value in let statement")?;
        Ok(Stmt::Let { name, value })
    }

    fn parse_expr_or_assignment_stmt(&mut self) -> Result<Stmt, FarmError> {
        let expr = self.collect_expr_until_semicolon("expected `;` after expression statement")?;
        self.require_expr(&expr, "expected expression")?;
        let Some(index) = top_level_assignment_index(&expr) else {
            return Ok(Stmt::Expr(expr));
        };

        let target = Expr::new(expr.tokens[..index].to_vec());
        let value = Expr::new(expr.tokens[index + 1..].to_vec());

        if target.is_empty() || value.is_empty() {
            let token = self.previous();
            return Err(FarmError::new(
                "invalid assignment statement",
                token.line,
                token.column,
            ));
        }

        Ok(Stmt::Assign { target, value })
    }

    fn collect_expr_until_semicolon(&mut self, missing_message: &str) -> Result<Expr, FarmError> {
        let mut tokens = Vec::new();
        let mut depth = 0usize;

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol(';') if depth == 0 => {
                    self.advance();
                    return Ok(Expr::new(tokens));
                }
                TokenKind::Symbol('}') if depth == 0 => {
                    return Err(FarmError::new(missing_message, token.line, token.column));
                }
                TokenKind::Comment(_) => {
                    return Err(FarmError::new(missing_message, token.line, token.column));
                }
                TokenKind::Symbol('(') => depth += 1,
                TokenKind::Symbol(')') => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }

            if let Some(expr_token) = token.expr_token() {
                tokens.push(expr_token);
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(FarmError::new(missing_message, line, column))
    }

    fn collect_expr_until_symbol(
        &mut self,
        symbol: char,
        missing_message: &str,
    ) -> Result<Expr, FarmError> {
        let mut tokens = Vec::new();
        let mut depth = 0usize;

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol(value) if *value == symbol && depth == 0 => {
                    return Ok(Expr::new(tokens));
                }
                TokenKind::Symbol('(') => depth += 1,
                TokenKind::Symbol(')') => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Symbol(';') | TokenKind::Symbol('}') if depth == 0 => {
                    return Err(FarmError::new(missing_message, token.line, token.column));
                }
                TokenKind::Comment(_) => {
                    return Err(FarmError::new(missing_message, token.line, token.column));
                }
                _ => {}
            }

            if let Some(expr_token) = token.expr_token() {
                tokens.push(expr_token);
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(FarmError::new(missing_message, line, column))
    }

    fn collect_expr_until_operator(
        &mut self,
        operator: &str,
        missing_message: &str,
    ) -> Result<Expr, FarmError> {
        let mut tokens = Vec::new();
        let mut depth = 0usize;

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Operator(value) if value == operator && depth == 0 => {
                    return Ok(Expr::new(tokens));
                }
                TokenKind::Symbol('(') => depth += 1,
                TokenKind::Symbol(')') => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Symbol(';') | TokenKind::Symbol('}') if depth == 0 => {
                    return Err(FarmError::new(missing_message, token.line, token.column));
                }
                TokenKind::Comment(_) => {
                    return Err(FarmError::new(missing_message, token.line, token.column));
                }
                _ => {}
            }

            if let Some(expr_token) = token.expr_token() {
                tokens.push(expr_token);
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(FarmError::new(missing_message, line, column))
    }

    fn require_expr(&self, expr: &Expr, message: &str) -> Result<(), FarmError> {
        if !expr.is_empty() {
            return Ok(());
        }

        let token = self.peek().or_else(|| self.tokens.last());
        let (line, column) = token
            .map(|t| (t.line, t.column))
            .unwrap_or_else(|| self.end_position());
        Err(FarmError::new(message, line, column))
    }

    fn reject_unsupported_starter(&self) -> Result<(), FarmError> {
        let Some(token) = self.peek() else {
            return Ok(());
        };

        if let TokenKind::Ident(value) = &token.kind {
            if matches!(
                value.as_str(),
                "trait"
                    | "impl"
                    | "macro"
                    | "macro_rules"
                    | "mod"
                    | "use"
                    | "struct"
                    | "enum"
                    | "crate"
            ) {
                return Err(FarmError::unsupported(value, token.line, token.column));
            }
        }

        Ok(())
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), FarmError> {
        if self.match_keyword(keyword) {
            Ok(())
        } else {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            Err(FarmError::new(
                format!("expected `{keyword}`"),
                line,
                column,
            ))
        }
    }

    fn expect_ident(&mut self, message: &str) -> Result<String, FarmError> {
        let Some(token) = self.peek() else {
            let (line, column) = self.end_position();
            return Err(FarmError::new(message, line, column));
        };

        if let TokenKind::Ident(value) = &token.kind {
            let value = value.clone();
            self.advance();
            Ok(value)
        } else {
            Err(FarmError::new(message, token.line, token.column))
        }
    }

    fn expect_symbol(&mut self, symbol: char) -> Result<(), FarmError> {
        if self.match_symbol(symbol) {
            Ok(())
        } else {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            Err(FarmError::new(format!("expected `{symbol}`"), line, column))
        }
    }

    fn expect_operator(&mut self, operator: &str) -> Result<(), FarmError> {
        if self.match_operator(operator) {
            Ok(())
        } else {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            Err(FarmError::new(
                format!("expected `{operator}`"),
                line,
                column,
            ))
        }
    }

    fn expect_semicolon(&mut self, message: &str) -> Result<(), FarmError> {
        if self.match_symbol(';') {
            Ok(())
        } else {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            Err(FarmError::new(message, line, column))
        }
    }

    fn match_keyword(&mut self, keyword: &str) -> bool {
        if self.check_keyword(keyword) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_symbol(&mut self, symbol: char) -> bool {
        if self.check_symbol(symbol) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_operator(&mut self, operator: &str) -> bool {
        if self.check_operator(operator) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_comment(&mut self) -> Option<String> {
        let Some(token) = self.peek() else {
            return None;
        };

        if let TokenKind::Comment(value) = &token.kind {
            let value = value.clone();
            self.advance();
            Some(value)
        } else {
            None
        }
    }

    fn check_keyword(&self, keyword: &str) -> bool {
        matches!(self.peek().map(|token| &token.kind), Some(TokenKind::Ident(value)) if value == keyword)
    }

    fn check_symbol(&self, symbol: char) -> bool {
        matches!(self.peek().map(|token| &token.kind), Some(TokenKind::Symbol(value)) if *value == symbol)
    }

    fn check_operator(&self, operator: &str) -> bool {
        matches!(self.peek().map(|token| &token.kind), Some(TokenKind::Operator(value)) if value == operator)
    }

    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn previous(&self) -> &'a Token {
        &self.tokens[self.pos.saturating_sub(1)]
    }

    fn advance(&mut self) {
        if !self.is_eof() {
            self.pos += 1;
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn end_position(&self) -> (usize, usize) {
        self.tokens
            .last()
            .map(|token| (token.line, token.column))
            .unwrap_or((1, 1))
    }
}

fn top_level_assignment_index(expr: &Expr) -> Option<usize> {
    let mut depth = 0usize;

    for (index, token) in expr.tokens.iter().enumerate() {
        match token {
            crate::ir::ExprToken::Symbol('(') => depth += 1,
            crate::ir::ExprToken::Symbol(')') => depth = depth.saturating_sub(1),
            crate::ir::ExprToken::Operator(op) if op == "=" && depth == 0 => return Some(index),
            _ => {}
        }
    }

    None
}
