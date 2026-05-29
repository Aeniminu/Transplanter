use crate::error::LispToPythonError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    LParen,
    RParen,
    String(String),
    Symbol(String),
}

pub fn lex(source: &str) -> Result<Vec<Token>, LispToPythonError> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();
    let mut line = 1;
    let mut column = 1;

    while let Some(ch) = chars.peek().copied() {
        match ch {
            '(' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::LParen,
                    line,
                    column,
                });
                column += 1;
            }
            ')' => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::RParen,
                    line,
                    column,
                });
                column += 1;
            }
            '"' => {
                tokens.push(read_string(&mut chars, &mut line, &mut column)?);
            }
            ';' => {
                while let Some(comment_ch) = chars.peek().copied() {
                    if comment_ch == '\n' {
                        break;
                    }
                    chars.next();
                    column += 1;
                }
            }
            '\n' => {
                chars.next();
                line += 1;
                column = 1;
            }
            ch if ch.is_whitespace() => {
                chars.next();
                column += 1;
            }
            _ => tokens.push(read_symbol(&mut chars, &mut column, line)),
        }
    }

    Ok(tokens)
}

fn read_string(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    line: &mut usize,
    column: &mut usize,
) -> Result<Token, LispToPythonError> {
    let start_line = *line;
    let start_column = *column;
    chars.next();
    *column += 1;

    let mut value = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                *column += 1;
                return Ok(Token {
                    kind: TokenKind::String(value),
                    line: start_line,
                    column: start_column,
                });
            }
            '\\' => {
                *column += 1;
                let Some(escaped) = chars.next() else {
                    return Err(LispToPythonError::new(
                        "文字列の escape が途中で終わっています",
                        start_line,
                        start_column,
                    ));
                };
                match escaped {
                    'n' => value.push('\n'),
                    'r' => value.push('\r'),
                    't' => value.push('\t'),
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    other => value.push(other),
                }
                *column += 1;
            }
            '\n' => {
                value.push('\n');
                *line += 1;
                *column = 1;
            }
            other => {
                value.push(other);
                *column += 1;
            }
        }
    }

    Err(LispToPythonError::new(
        "文字列が閉じられていません",
        start_line,
        start_column,
    ))
}

fn read_symbol(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    column: &mut usize,
    line: usize,
) -> Token {
    let start_column = *column;
    let mut value = String::new();

    while let Some(ch) = chars.peek().copied() {
        if ch.is_whitespace() || ch == '(' || ch == ')' || ch == ';' {
            break;
        }
        value.push(ch);
        chars.next();
        *column += 1;
    }

    Token {
        kind: TokenKind::Symbol(value),
        line,
        column: start_column,
    }
}
