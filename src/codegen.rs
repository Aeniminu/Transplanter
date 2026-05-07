use crate::api_map;
use crate::ir::{ElseBranch, Expr, ExprToken, Program, Stmt};

pub fn generate(program: &Program) -> String {
    let mut lines = Vec::new();

    for function in &program.functions {
        let params = function.params.join(", ");
        lines.push(format!("def {}({}):", function.name, params));
        emit_block(&mut lines, &function.body, 1);
        lines.push(String::new());
    }

    emit_block(&mut lines, &program.main, 0);

    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    let mut output = lines.join("\n");
    output.push('\n');
    output
}

fn emit_block(lines: &mut Vec<String>, body: &[Stmt], indent: usize) {
    let has_code = body.iter().any(|stmt| !matches!(stmt, Stmt::Comment(_)));

    for stmt in body {
        emit_stmt(lines, stmt, indent);
    }

    if !has_code {
        lines.push(format!("{}pass", indentation(indent)));
    }
}

fn emit_stmt(lines: &mut Vec<String>, stmt: &Stmt, indent: usize) {
    let prefix = indentation(indent);

    match stmt {
        Stmt::Comment(text) => {
            let trimmed = text.trim_start();
            if trimmed.is_empty() {
                lines.push(format!("{prefix}#"));
            } else {
                lines.push(format!("{prefix}# {trimmed}"));
            }
        }
        Stmt::Loop(body) => {
            lines.push(format!("{prefix}while True:"));
            emit_block(lines, body, indent + 1);
        }
        Stmt::While { condition, body } => {
            lines.push(format!("{prefix}while {}:", format_expr(condition)));
            emit_block(lines, body, indent + 1);
        }
        Stmt::If {
            condition,
            then_body,
            else_branch,
        } => {
            lines.push(format!("{prefix}if {}:", format_expr(condition)));
            emit_block(lines, then_body, indent + 1);
            emit_else_branch(lines, else_branch.as_ref(), indent);
        }
        Stmt::For {
            variable,
            start,
            end,
            body,
        } => {
            let start = format_expr(start);
            let end = format_expr(end);
            let range = if start == "0" {
                format!("range({end})")
            } else {
                format!("range({start}, {end})")
            };
            lines.push(format!("{prefix}for {variable} in {range}:"));
            emit_block(lines, body, indent + 1);
        }
        Stmt::Let { name, value } => {
            lines.push(format!("{prefix}{name} = {}", format_expr(value)));
        }
        Stmt::Assign { target, value } => {
            lines.push(format!(
                "{prefix}{} = {}",
                format_expr(target),
                format_expr(value)
            ));
        }
        Stmt::Expr(expr) => lines.push(format!("{prefix}{}", format_expr(expr))),
        Stmt::Break => lines.push(format!("{prefix}break")),
        Stmt::Continue => lines.push(format!("{prefix}continue")),
        Stmt::Return(None) => lines.push(format!("{prefix}return")),
        Stmt::Return(Some(expr)) => lines.push(format!("{prefix}return {}", format_expr(expr))),
    }
}

fn emit_else_branch(lines: &mut Vec<String>, else_branch: Option<&ElseBranch>, indent: usize) {
    let Some(else_branch) = else_branch else {
        return;
    };

    let prefix = indentation(indent);
    match else_branch {
        ElseBranch::ElseIf {
            condition,
            then_body,
            else_branch,
        } => {
            lines.push(format!("{prefix}elif {}:", format_expr(condition)));
            emit_block(lines, then_body, indent + 1);
            emit_else_branch(lines, else_branch.as_deref(), indent);
        }
        ElseBranch::Else(body) => {
            lines.push(format!("{prefix}else:"));
            emit_block(lines, body, indent + 1);
        }
    }
}

fn format_expr(expr: &Expr) -> String {
    let mut out = String::new();
    let mut i = 0usize;

    while i < expr.tokens.len() {
        if let Some((mapped, consumed)) = map_namespace_expr(&expr.tokens[i..]) {
            append_atom(&mut out, &mapped);
            i += consumed;
            continue;
        }

        match &expr.tokens[i] {
            ExprToken::Ident(value) => {
                append_atom(&mut out, &api_map::map_identifier(value));
            }
            ExprToken::Number(value) | ExprToken::String(value) => {
                append_atom(&mut out, value);
            }
            ExprToken::Symbol('(') => {
                trim_trailing_spaces(&mut out);
                out.push('(');
            }
            ExprToken::Symbol(')') => {
                trim_trailing_spaces(&mut out);
                out.push(')');
            }
            ExprToken::Symbol(',') => {
                trim_trailing_spaces(&mut out);
                out.push_str(", ");
            }
            ExprToken::Symbol(symbol) => {
                append_atom(&mut out, &symbol.to_string());
            }
            ExprToken::Operator(value) => match value.as_str() {
                "&&" => append_binary_word(&mut out, "and"),
                "||" => append_binary_word(&mut out, "or"),
                "!" => append_prefix_word(&mut out, "not"),
                "." => {
                    trim_trailing_spaces(&mut out);
                    out.push('.');
                }
                "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "=" => {
                    append_binary_operator(&mut out, value);
                }
                other => append_atom(&mut out, other),
            },
        }

        i += 1;
    }

    out.trim().to_string()
}

fn map_namespace_expr(tokens: &[ExprToken]) -> Option<(String, usize)> {
    match tokens {
        [
            ExprToken::Ident(namespace),
            ExprToken::Operator(operator),
            ExprToken::Ident(name),
            ..,
        ] if operator == "::" => Some((api_map::map_namespace(namespace, name), 3)),
        _ => None,
    }
}

fn append_atom(out: &mut String, text: &str) {
    if should_insert_space_before_atom(out) {
        out.push(' ');
    }
    out.push_str(text);
}

fn append_binary_word(out: &mut String, text: &str) {
    trim_trailing_spaces(out);
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(text);
    out.push(' ');
}

fn append_prefix_word(out: &mut String, text: &str) {
    if should_insert_space_before_atom(out) {
        out.push(' ');
    }
    out.push_str(text);
    out.push(' ');
}

fn append_binary_operator(out: &mut String, text: &str) {
    trim_trailing_spaces(out);
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(text);
    out.push(' ');
}

fn should_insert_space_before_atom(out: &str) -> bool {
    let Some(last) = out.chars().last() else {
        return false;
    };

    !matches!(last, '(' | '.' | ' ' | '\n')
}

fn trim_trailing_spaces(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
}

fn indentation(indent: usize) -> String {
    "    ".repeat(indent)
}
