use crate::error::FarmError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Ident(String),
    Number(String),
    String(String),
    Comment(String),
    Symbol(char),
    Operator(String),
}

impl Token {
    pub fn expr_token(&self) -> Option<crate::ir::ExprToken> {
        match &self.kind {
            TokenKind::Ident(value) => Some(crate::ir::ExprToken::Ident(value.clone())),
            TokenKind::Number(value) => Some(crate::ir::ExprToken::Number(value.clone())),
            TokenKind::String(value) => Some(crate::ir::ExprToken::String(value.clone())),
            TokenKind::Symbol(value) => Some(crate::ir::ExprToken::Symbol(*value)),
            TokenKind::Operator(value) => Some(crate::ir::ExprToken::Operator(value.clone())),
            TokenKind::Comment(_) => None,
        }
    }
}

pub fn lex(source: &str) -> Result<Vec<Token>, FarmError> {
    let chars: Vec<char> = source.chars().collect();
    let mut lexer = Lexer {
        chars,
        pos: 0,
        line: 1,
        column: 1,
        tokens: Vec::new(),
    };
    lexer.lex_all()?;
    Ok(lexer.tokens)
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
}

impl Lexer {
    fn lex_all(&mut self) -> Result<(), FarmError> {
        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => {
                    self.advance();
                }
                '/' if self.peek_next() == Some('/') => {
                    if self.is_floor_div_operator() {
                        self.lex_operator()?;
                    } else {
                        self.lex_comment();
                    }
                }
                '/' if self.peek_next() == Some('*') => self.lex_block_comment()?,
                '"' => self.lex_string()?,
                '\'' if self.peek_next().is_some_and(is_ident_start) => self.lex_lifetime(),
                '0'..='9' => self.lex_number(),
                'a'..='z' | 'A'..='Z' | '_' => self.lex_ident(),
                ':' if self.peek_next() == Some(':') => self.lex_operator()?,
                '{' | '}' | '(' | ')' | '[' | ']' | ',' | ';' | ':' => {
                    let line = self.line;
                    let column = self.column;
                    let symbol = self.advance().expect("peeked character exists");
                    self.push(TokenKind::Symbol(symbol), line, column);
                }
                '=' | '!' | '<' | '>' | '&' | '|' | '.' | '-' | '+' | '*' | '/' | '%' => {
                    self.lex_operator()?;
                }
                _ => {
                    return Err(FarmError::new(
                        format!("予期しない文字 `{ch}`"),
                        self.line,
                        self.column,
                    ));
                }
            }
        }

        Ok(())
    }

    fn lex_comment(&mut self) {
        let line = self.line;
        let column = self.column;
        self.advance();
        self.advance();

        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                break;
            }
            value.push(ch);
            self.advance();
        }

        self.push(TokenKind::Comment(value), line, column);
    }

    fn lex_block_comment(&mut self) -> Result<(), FarmError> {
        let line = self.line;
        let column = self.column;
        self.advance();
        self.advance();

        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch == '*' && self.peek_next() == Some('/') {
                self.advance();
                self.advance();
                self.push(TokenKind::Comment(value), line, column);
                return Ok(());
            }

            value.push(ch);
            self.advance();
        }

        Err(FarmError::new(
            "ブロックコメントが閉じられていません",
            line,
            column,
        ))
    }

    fn lex_string(&mut self) -> Result<(), FarmError> {
        let line = self.line;
        let column = self.column;
        let mut value = String::new();
        value.push(self.advance().expect("peeked character exists"));

        let mut escaped = false;
        while let Some(ch) = self.peek() {
            value.push(ch);
            self.advance();

            if escaped {
                escaped = false;
                continue;
            }

            match ch {
                '\\' => escaped = true,
                '"' => {
                    self.push(TokenKind::String(value), line, column);
                    return Ok(());
                }
                '\n' => {
                    return Err(FarmError::new(
                        "文字列リテラルが閉じられていません",
                        line,
                        column,
                    ));
                }
                _ => {}
            }
        }

        Err(FarmError::new(
            "文字列リテラルが閉じられていません",
            line,
            column,
        ))
    }

    fn lex_number(&mut self) {
        let line = self.line;
        let column = self.column;
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '_' {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        self.push(TokenKind::Number(value), line, column);
    }

    fn lex_ident(&mut self) {
        let line = self.line;
        let column = self.column;
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        self.push(TokenKind::Ident(value), line, column);
    }

    fn lex_lifetime(&mut self) {
        let line = self.line;
        let column = self.column;
        let mut value = String::new();
        value.push(self.advance().expect("peeked character exists"));

        while let Some(ch) = self.peek() {
            if is_ident_continue(ch) {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        self.push(TokenKind::Ident(value), line, column);
    }

    fn lex_operator(&mut self) -> Result<(), FarmError> {
        let line = self.line;
        let column = self.column;
        let ch = self.advance().expect("peeked character exists");
        let next = self.peek();

        let op = match (ch, next) {
            ('/', Some('/')) => {
                self.advance();
                "//"
            }
            ('*', Some('*')) => {
                self.advance();
                "**"
            }
            ('=', Some('=')) => {
                self.advance();
                "=="
            }
            ('!', Some('=')) => {
                self.advance();
                "!="
            }
            ('<', Some('=')) => {
                self.advance();
                "<="
            }
            ('>', Some('=')) => {
                self.advance();
                ">="
            }
            ('&', Some('&')) => {
                self.advance();
                "&&"
            }
            ('|', Some('|')) => {
                self.advance();
                "||"
            }
            (':', Some(':')) => {
                self.advance();
                "::"
            }
            ('.', Some('.')) => {
                self.advance();
                ".."
            }
            ('-', Some('>')) => {
                self.advance();
                "->"
            }
            ('|', _) => return Err(FarmError::new("`||` が必要です", line, column)),
            _ => {
                let text = ch.to_string();
                self.push(TokenKind::Operator(text), line, column);
                return Ok(());
            }
        };

        self.push(TokenKind::Operator(op.to_string()), line, column);
        Ok(())
    }

    fn is_floor_div_operator(&self) -> bool {
        if self.peek() != Some('/') || self.peek_next() != Some('/') {
            return false;
        }

        let Some(prev) = self.previous_non_space_on_line() else {
            return false;
        };

        if matches!(prev, ';' | '{' | '(' | '[' | ',' | ':') {
            return false;
        }

        let Some(next) = self.next_non_space_after(2) else {
            return false;
        };

        if next == '\n' {
            return false;
        }

        self.line_contains_expr_terminator_after(2)
    }

    fn previous_non_space_on_line(&self) -> Option<char> {
        let mut pos = self.pos;
        while pos > 0 {
            pos -= 1;
            let ch = self.chars[pos];
            if ch == '\n' {
                return None;
            }
            if !matches!(ch, ' ' | '\t' | '\r') {
                return Some(ch);
            }
        }
        None
    }

    fn next_non_space_after(&self, offset: usize) -> Option<char> {
        let mut pos = self.pos + offset;
        while let Some(ch) = self.chars.get(pos).copied() {
            if !matches!(ch, ' ' | '\t' | '\r') {
                return Some(ch);
            }
            pos += 1;
        }
        None
    }

    fn line_contains_expr_terminator_after(&self, offset: usize) -> bool {
        let mut pos = self.pos + offset;
        while let Some(ch) = self.chars.get(pos).copied() {
            if ch == '\n' {
                return false;
            }
            if matches!(ch, ';' | '{' | ')' | ']' | ',') {
                return true;
            }
            pos += 1;
        }
        false
    }

    fn push(&mut self, kind: TokenKind, line: usize, column: usize) {
        self.tokens.push(Token { kind, line, column });
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied()?;
        self.pos += 1;

        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        Some(ch)
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
