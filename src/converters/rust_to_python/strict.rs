use super::error::RustToPythonError;
use super::lexer::{Token, TokenKind};

pub fn validate_expr(tokens: &[Token]) -> Result<(), RustToPythonError> {
    for (index, token) in tokens.iter().enumerate() {
        match &token.kind {
            TokenKind::Ident(value) => {
                validate_ident(tokens, index, token, value)?;
            }
            TokenKind::Operator(value) if value == "//" => {
                return Err(RustToPythonError::new(
                    "Python風の `//` 除算は入力では使えません。Rustとして成立する式にしてください",
                    token.line,
                    token.column,
                ));
            }
            TokenKind::Operator(value) if value == "**" => {
                return Err(RustToPythonError::new(
                    "`**` はRustの演算子ではありません",
                    token.line,
                    token.column,
                ));
            }
            TokenKind::Symbol('{') if !is_rust_struct_literal_start(tokens, index) => {
                return Err(RustToPythonError::new(
                    "Python風の dict/set リテラルは入力では使えません。`dict()` と添字代入を使ってください",
                    token.line,
                    token.column,
                ));
            }
            _ => {}
        }
    }

    Ok(())
}

fn validate_ident(
    tokens: &[Token],
    index: usize,
    token: &Token,
    value: &str,
) -> Result<(), RustToPythonError> {
    match value {
        "True" => {
            return Err(RustToPythonError::new(
                "Python風の `True` は使えません。Rustの `true` を使ってください。常に繰り返す場合は `loop { ... }` も使えます",
                token.line,
                token.column,
            ));
        }
        "False" => {
            return Err(RustToPythonError::new(
                "Python風の `False` は使えません。Rustの `false` を使ってください",
                token.line,
                token.column,
            ));
        }
        "and" => {
            return Err(RustToPythonError::new(
                "Python風の `and` は使えません。Rustの `&&` を使ってください",
                token.line,
                token.column,
            ));
        }
        "or" => {
            return Err(RustToPythonError::new(
                "Python風の `or` は使えません。Rustの `||` を使ってください",
                token.line,
                token.column,
            ));
        }
        "not" => {
            return Err(RustToPythonError::new(
                "Python風の `not` は使えません。Rustの `!` を使ってください",
                token.line,
                token.column,
            ));
        }
        "range" if next_is_symbol(tokens, index, '(') => {
            return Err(RustToPythonError::new(
                "`range(...)` は入力では使えません。`for i in 0..n { ... }` のようにRustの範囲構文を使ってください",
                token.line,
                token.column,
            ));
        }
        "move" if next_is_symbol(tokens, index, '(') => {
            return Err(RustToPythonError::new(
                "`move(...)` は出力側の書き方です。入力では `move_dir(Direction::North)` を使ってください",
                token.line,
                token.column,
            ));
        }
        direction
            if is_direction_ident(direction) && !is_direction_namespace_member(tokens, index) =>
        {
            return Err(RustToPythonError::new(
                format!(
                    "`{direction}` は出力側の書き方です。入力では `Direction::{direction}` を使ってください"
                ),
                token.line,
                token.column,
            ));
        }
        namespace
            if is_python_output_namespace(namespace)
                && next_is_namespace_separator(tokens, index) =>
        {
            return Err(RustToPythonError::new(
                format!(
                    "`{namespace}` は出力側の名前空間です。入力では `{}` を使ってください",
                    singular_namespace(namespace)
                ),
                token.line,
                token.column,
            ));
        }
        namespace if is_game_namespace(namespace) && next_is_operator(tokens, index, ".") => {
            return Err(RustToPythonError::new(
                format!(
                    "`{namespace}.X` はRustの名前空間構文ではありません。`{namespace}::X` を使ってください"
                ),
                token.line,
                token.column,
            ));
        }
        _ => {}
    }

    Ok(())
}

fn next_is_symbol(tokens: &[Token], index: usize, expected: char) -> bool {
    matches!(
        tokens.get(index + 1).map(|token| &token.kind),
        Some(TokenKind::Symbol(value)) if *value == expected
    )
}

fn next_is_operator(tokens: &[Token], index: usize, expected: &str) -> bool {
    matches!(
        tokens.get(index + 1).map(|token| &token.kind),
        Some(TokenKind::Operator(value)) if value == expected
    )
}

fn next_is_namespace_separator(tokens: &[Token], index: usize) -> bool {
    next_is_operator(tokens, index, ".") || next_is_operator(tokens, index, "::")
}

fn is_direction_namespace_member(tokens: &[Token], index: usize) -> bool {
    if index < 2 {
        return false;
    }

    matches!(
        (&tokens[index - 2].kind, &tokens[index - 1].kind),
        (TokenKind::Ident(namespace), TokenKind::Operator(separator))
            if namespace == "Direction" && separator == "::"
    )
}

fn is_direction_ident(value: &str) -> bool {
    matches!(value, "North" | "East" | "South" | "West")
}

fn is_game_namespace(value: &str) -> bool {
    matches!(
        value,
        "Entity" | "Ground" | "Item" | "Unlock" | "Leaderboard"
    )
}

fn is_python_output_namespace(value: &str) -> bool {
    matches!(
        value,
        "Entities" | "Grounds" | "Items" | "Unlocks" | "Leaderboards"
    )
}

fn singular_namespace(value: &str) -> &str {
    match value {
        "Entities" => "Entity",
        "Grounds" => "Ground",
        "Items" => "Item",
        "Unlocks" => "Unlock",
        "Leaderboards" => "Leaderboard",
        _ => value,
    }
}

fn is_rust_struct_literal_start(tokens: &[Token], index: usize) -> bool {
    index > 0 && matches!(&tokens[index - 1].kind, TokenKind::Ident(_))
}
