use std::collections::{BTreeMap, BTreeSet};

use crate::api_map;
use crate::ir::{
    ElseBranch, Expr, ExprToken, FarmIr, Function, FunctionParam, Stmt, StructFactory,
};

pub fn generate(program: &FarmIr) -> String {
    let mut lines = Vec::new();
    let context = CodegenContext::new(program);

    for constant in &program.constants {
        lines.push(format!("{} = {:?}", constant.name, constant.value));
    }

    if !program.constants.is_empty()
        && (!program.struct_factories.is_empty() || !program.functions.is_empty())
    {
        lines.push(String::new());
    }

    for factory in &program.struct_factories {
        emit_struct_factory(&mut lines, factory);
        lines.push(String::new());
    }

    for function in &program.functions {
        emit_function(&mut lines, function, 0, &context);
        lines.push(String::new());
    }

    emit_block(&mut lines, &program.main, 0, &context);

    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    let mut output = lines.join("\n");
    output.push('\n');
    output
}

struct CodegenContext {
    namespace_aliases: BTreeMap<String, String>,
    struct_names: BTreeSet<String>,
}

impl CodegenContext {
    fn new(program: &FarmIr) -> Self {
        Self {
            namespace_aliases: program
                .namespace_aliases
                .iter()
                .map(|alias| (namespace_key(&alias.path), alias.output.clone()))
                .collect(),
            struct_names: program
                .struct_factories
                .iter()
                .map(|factory| factory.name.clone())
                .collect(),
        }
    }
}

fn namespace_key(path: &[String]) -> String {
    path.join("::")
}

fn emit_struct_factory(lines: &mut Vec<String>, factory: &StructFactory) {
    let params = factory
        .fields
        .iter()
        .map(|field| format!("{field}=None"))
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(format!("def {}({}):", factory.name, params));

    if factory.fields.is_empty() {
        lines.push("    return dict()".to_string());
        return;
    }

    let entries = factory
        .fields
        .iter()
        .map(|field| format!("{field:?}: {field}"))
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(format!("    return {{{entries}}}"));
}

fn emit_block(lines: &mut Vec<String>, body: &[Stmt], indent: usize, context: &CodegenContext) {
    let has_code = body
        .iter()
        .any(|stmt| !matches!(stmt, Stmt::Comment(_) | Stmt::Noop));

    for stmt in body {
        emit_stmt(lines, stmt, indent, context);
    }

    if !has_code {
        lines.push(format!("{}pass", indentation(indent)));
    }
}

fn emit_stmt(lines: &mut Vec<String>, stmt: &Stmt, indent: usize, context: &CodegenContext) {
    let prefix = indentation(indent);

    match stmt {
        Stmt::Noop => {}
        Stmt::Comment(text) => {
            for line in format_comment_lines(text) {
                lines.push(format!("{prefix}{line}"));
            }
        }
        Stmt::Function(function) => emit_function(lines, function, indent, context),
        Stmt::Loop(body) => {
            lines.push(format!("{prefix}while True:"));
            emit_block(lines, body, indent + 1, context);
        }
        Stmt::While { condition, body } => {
            lines.push(format!(
                "{prefix}while {}:",
                format_expr(condition, context)
            ));
            emit_block(lines, body, indent + 1, context);
        }
        Stmt::If {
            condition,
            then_body,
            else_branch,
        } => {
            lines.push(format!("{prefix}if {}:", format_expr(condition, context)));
            emit_block(lines, then_body, indent + 1, context);
            emit_else_branch(lines, else_branch.as_ref(), indent, context);
        }
        Stmt::For {
            variable,
            start,
            end,
            body,
        } => {
            let start = format_expr(start, context);
            let end = format_expr(end, context);
            let range = if start == "0" {
                format!("range({end})")
            } else {
                format!("range({start}, {end})")
            };
            lines.push(format!("{prefix}for {variable} in {range}:"));
            emit_block(lines, body, indent + 1, context);
        }
        Stmt::ForEach {
            variable,
            iterable,
            body,
        } => {
            lines.push(format!(
                "{prefix}for {variable} in {}:",
                format_expr(iterable, context)
            ));
            emit_block(lines, body, indent + 1, context);
        }
        Stmt::Let { name, value } => {
            lines.push(format!("{prefix}{name} = {}", format_expr(value, context)));
        }
        Stmt::Assign { target, value } => {
            lines.push(format!(
                "{prefix}{} = {}",
                format_expr(target, context),
                format_expr(value, context)
            ));
        }
        Stmt::Expr(expr) => lines.push(format!("{prefix}{}", format_expr(expr, context))),
        Stmt::Break => lines.push(format!("{prefix}break")),
        Stmt::Continue => lines.push(format!("{prefix}continue")),
        Stmt::Return(None) => lines.push(format!("{prefix}return")),
        Stmt::Return(Some(expr)) => {
            lines.push(format!("{prefix}return {}", format_expr(expr, context)));
        }
    }
}

fn emit_function(
    lines: &mut Vec<String>,
    function: &Function,
    indent: usize,
    context: &CodegenContext,
) {
    let prefix = indentation(indent);
    let params = function
        .params
        .iter()
        .map(|param| format_param(param, context))
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(format!("{prefix}def {}({}):", function.name, params));
    emit_block(lines, &function.body, indent + 1, context);
}

fn format_param(param: &FunctionParam, context: &CodegenContext) -> String {
    match &param.default {
        Some(default) => format!("{}={}", param.name, format_expr(default, context)),
        None => param.name.clone(),
    }
}

fn format_comment_lines(text: &str) -> Vec<String> {
    let mut lines = text
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                "#".to_string()
            } else {
                format!("# {trimmed}")
            }
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        lines.push("#".to_string());
    }

    lines
}

fn emit_else_branch(
    lines: &mut Vec<String>,
    else_branch: Option<&ElseBranch>,
    indent: usize,
    context: &CodegenContext,
) {
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
            lines.push(format!("{prefix}elif {}:", format_expr(condition, context)));
            emit_block(lines, then_body, indent + 1, context);
            emit_else_branch(lines, else_branch.as_deref(), indent, context);
        }
        ElseBranch::Else(body) => {
            lines.push(format!("{prefix}else:"));
            emit_block(lines, body, indent + 1, context);
        }
    }
}

fn format_expr(expr: &Expr, context: &CodegenContext) -> String {
    let mut out = String::new();
    let mut i = 0usize;

    while i < expr.tokens.len() {
        if let Some((mapped, consumed)) = map_namespace_expr(&expr.tokens[i..], context) {
            append_atom(&mut out, &mapped);
            i += consumed;
            continue;
        }

        if let Some((mapped, consumed)) = format_struct_literal(&expr.tokens[i..], context) {
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
            ExprToken::Symbol('[') => {
                out.push('[');
            }
            ExprToken::Symbol(']') => {
                trim_trailing_spaces(&mut out);
                out.push(']');
            }
            ExprToken::Symbol('{') => {
                out.push('{');
            }
            ExprToken::Symbol('}') => {
                trim_trailing_spaces(&mut out);
                out.push('}');
            }
            ExprToken::Symbol(',') => {
                trim_trailing_spaces(&mut out);
                out.push_str(", ");
            }
            ExprToken::Symbol(':') => {
                trim_trailing_spaces(&mut out);
                out.push_str(": ");
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
                "+" | "-" | "*" | "/" | "//" | "%" | "**" | "==" | "!=" | "<" | "<=" | ">"
                | ">=" | "=" => append_binary_operator(&mut out, value),
                other => append_atom(&mut out, other),
            },
        }

        i += 1;
    }

    out.trim().to_string()
}

fn map_namespace_expr(tokens: &[ExprToken], context: &CodegenContext) -> Option<(String, usize)> {
    let ExprToken::Ident(first) = tokens.first()? else {
        return None;
    };

    let mut parts = vec![first.clone()];
    let mut consumed = 1usize;

    while matches!(tokens.get(consumed), Some(ExprToken::Operator(value)) if value == "::") {
        let Some(ExprToken::Ident(name)) = tokens.get(consumed + 1) else {
            break;
        };
        parts.push(name.clone());
        consumed += 2;
    }

    if parts.len() < 2 {
        return None;
    }

    if let Some(alias) = context.namespace_aliases.get(&namespace_key(&parts)) {
        return Some((alias.clone(), consumed));
    }

    if parts.len() == 2 {
        return Some((api_map::map_namespace(&parts[0], &parts[1]), consumed));
    }

    Some((parts.join("."), consumed))
}

fn format_struct_literal(
    tokens: &[ExprToken],
    context: &CodegenContext,
) -> Option<(String, usize)> {
    let [ExprToken::Ident(name), ExprToken::Symbol('{'), ..] = tokens else {
        return None;
    };

    if !context.struct_names.contains(name) {
        return None;
    }

    let close = matching_symbol_index(tokens, 1, '{', '}')?;
    let inner = &tokens[2..close];
    let fields = split_top_level(inner, ',')
        .into_iter()
        .map(|field| format_struct_field(&field, context))
        .collect::<Option<Vec<_>>>()?;

    Some((format!("{name}({})", fields.join(", ")), close + 1))
}

fn format_struct_field(tokens: &[ExprToken], context: &CodegenContext) -> Option<String> {
    let colon = top_level_symbol_index(tokens, ':')?;
    let [ExprToken::Ident(name)] = &tokens[..colon] else {
        return None;
    };

    let value = format_expr(&Expr::new(tokens[colon + 1..].to_vec()), context);
    Some(format!("{name}={value}"))
}

fn matching_symbol_index(
    tokens: &[ExprToken],
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;

    for (index, token) in tokens.iter().enumerate().skip(open_index) {
        match token {
            ExprToken::Symbol(value) if *value == open => depth += 1,
            ExprToken::Symbol(value) if *value == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }

    None
}

fn split_top_level(tokens: &[ExprToken], separator: char) -> Vec<Vec<ExprToken>> {
    let mut pieces = Vec::new();
    let mut current = Vec::new();
    let mut depth = ExprDepth::default();

    for token in tokens {
        if matches!(token, ExprToken::Symbol(value) if *value == separator) && depth.is_top_level()
        {
            pieces.push(current);
            current = Vec::new();
            continue;
        }

        depth.observe(token);
        current.push(token.clone());
    }

    if !current.is_empty() {
        pieces.push(current);
    }

    pieces
}

fn top_level_symbol_index(tokens: &[ExprToken], symbol: char) -> Option<usize> {
    let mut depth = ExprDepth::default();

    for (index, token) in tokens.iter().enumerate() {
        if matches!(token, ExprToken::Symbol(value) if *value == symbol) && depth.is_top_level() {
            return Some(index);
        }

        depth.observe(token);
    }

    None
}

#[derive(Default)]
struct ExprDepth {
    paren: usize,
    bracket: usize,
    brace: usize,
}

impl ExprDepth {
    fn is_top_level(&self) -> bool {
        self.paren == 0 && self.bracket == 0 && self.brace == 0
    }

    fn observe(&mut self, token: &ExprToken) {
        match token {
            ExprToken::Symbol('(') => self.paren += 1,
            ExprToken::Symbol(')') => self.paren = self.paren.saturating_sub(1),
            ExprToken::Symbol('[') => self.bracket += 1,
            ExprToken::Symbol(']') => self.bracket = self.bracket.saturating_sub(1),
            ExprToken::Symbol('{') => self.brace += 1,
            ExprToken::Symbol('}') => self.brace = self.brace.saturating_sub(1),
            _ => {}
        }
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

    !matches!(last, '(' | '[' | '{' | '.' | ' ' | '\n')
}

fn trim_trailing_spaces(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
}

fn indentation(indent: usize) -> String {
    "    ".repeat(indent)
}
