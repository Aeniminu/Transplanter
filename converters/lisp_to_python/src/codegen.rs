use crate::error::LispToPythonError;
use crate::ir::{Expr, ExprKind};

pub fn generate(forms: &[Expr]) -> Result<String, LispToPythonError> {
    let mut generator = Generator::default();
    generator.generate(forms)
}

#[derive(Default)]
struct Generator {
    output: String,
}

#[derive(Clone, Copy)]
enum BodyMode {
    AllowMainEntry,
    Function,
}

impl Generator {
    fn generate(&mut self, forms: &[Expr]) -> Result<String, LispToPythonError> {
        let mut main_body = None;
        let mut top_level = Vec::new();

        for form in forms {
            if is_use_form(form) {
                continue;
            }

            if let Some(function) = parse_function_define(form)? {
                if function.name == "main" {
                    main_body = Some(function.body);
                } else {
                    self.emit_function(&function.name, &function.params, &function.body, 0)?;
                    self.blank_line();
                }
                continue;
            }

            top_level.push(form.clone());
        }

        for form in &top_level {
            self.emit_statement(form, 0, BodyMode::AllowMainEntry)?;
        }

        if let Some(body) = main_body {
            for form in &body {
                self.emit_statement(form, 0, BodyMode::AllowMainEntry)?;
            }
        }

        Ok(trim_extra_blank_lines(&self.output))
    }

    fn emit_function(
        &mut self,
        name: &str,
        params: &[String],
        body: &[Expr],
        indent: usize,
    ) -> Result<(), LispToPythonError> {
        self.line(
            indent,
            &format!("def {}({}):", py_identifier(name), params.join(", ")),
        );
        if body.is_empty() {
            self.line(indent + 1, "pass");
            return Ok(());
        }
        for form in body {
            self.emit_statement(form, indent + 1, BodyMode::Function)?;
        }
        Ok(())
    }

    fn emit_statement(
        &mut self,
        expr: &Expr,
        indent: usize,
        mode: BodyMode,
    ) -> Result<(), LispToPythonError> {
        let ExprKind::List(items) = &expr.kind else {
            let value = self.emit_expr(expr)?;
            self.line(indent, &value);
            return Ok(());
        };

        if items.is_empty() {
            return Err(LispToPythonError::new(
                "空のリストは文として使えません",
                expr.line,
                expr.column,
            ));
        }

        let Some(head) = symbol_name(&items[0]) else {
            let value = self.emit_expr(expr)?;
            self.line(indent, &value);
            return Ok(());
        };

        match head {
            "begin" => self.emit_block(&items[1..], indent, mode),
            "define" => self.emit_define_statement(expr, items, indent),
            "for" => self.emit_for(expr, items, indent, mode),
            "if" => self.emit_if(expr, items, indent, mode),
            "let" => self.emit_let(expr, items, indent, mode),
            "loop" => self.emit_loop(&items[1..], indent, mode),
            "return" => self.emit_return(items, indent),
            "set!" => self.emit_set(expr, items, indent),
            "set-index!" => self.emit_set_index(expr, items, indent),
            "while" => self.emit_while(expr, items, indent, mode),
            _ => {
                let value = self.emit_expr(expr)?;
                self.line(indent, &value);
                Ok(())
            }
        }
    }

    fn emit_define_statement(
        &mut self,
        expr: &Expr,
        items: &[Expr],
        indent: usize,
    ) -> Result<(), LispToPythonError> {
        if let Some(function) = parse_function_define(expr)? {
            self.emit_function(&function.name, &function.params, &function.body, indent)
        } else if items.len() == 3 {
            let Some(name) = symbol_name(&items[1]) else {
                return Err(LispToPythonError::new(
                    "`define` の変数名は symbol で書いてください",
                    items[1].line,
                    items[1].column,
                ));
            };
            let value = self.emit_expr(&items[2])?;
            self.line(indent, &format!("{} = {value}", py_identifier(name)));
            Ok(())
        } else {
            Err(LispToPythonError::new(
                "`define` は `(define (name args...) ...)` または `(define name value)` で書いてください",
                expr.line,
                expr.column,
            ))
        }
    }

    fn emit_for(
        &mut self,
        expr: &Expr,
        items: &[Expr],
        indent: usize,
        mode: BodyMode,
    ) -> Result<(), LispToPythonError> {
        if items.len() < 5 {
            return Err(LispToPythonError::new(
                "`for` は `(for i start end body...)` で書いてください",
                expr.line,
                expr.column,
            ));
        }
        let Some(name) = symbol_name(&items[1]) else {
            return Err(LispToPythonError::new(
                "`for` の変数名は symbol で書いてください",
                items[1].line,
                items[1].column,
            ));
        };
        let start = self.emit_expr(&items[2])?;
        let end = self.emit_expr(&items[3])?;
        self.line(
            indent,
            &format!("for {} in range({start}, {end}):", py_identifier(name)),
        );
        self.emit_block_or_pass(&items[4..], indent + 1, mode)
    }

    fn emit_if(
        &mut self,
        expr: &Expr,
        items: &[Expr],
        indent: usize,
        mode: BodyMode,
    ) -> Result<(), LispToPythonError> {
        if !(3..=4).contains(&items.len()) {
            return Err(LispToPythonError::new(
                "`if` は `(if condition then [else])` で書いてください",
                expr.line,
                expr.column,
            ));
        }
        let condition = self.emit_expr(&items[1])?;
        self.line(indent, &format!("if {condition}:"));
        self.emit_statement_or_begin(&items[2], indent + 1, mode)?;
        if let Some(otherwise) = items.get(3) {
            self.line(indent, "else:");
            self.emit_statement_or_begin(otherwise, indent + 1, mode)?;
        }
        Ok(())
    }

    fn emit_let(
        &mut self,
        expr: &Expr,
        items: &[Expr],
        indent: usize,
        mode: BodyMode,
    ) -> Result<(), LispToPythonError> {
        if items.len() < 3 {
            return Err(LispToPythonError::new(
                "`let` は `(let ((name value) ...) body...)` で書いてください",
                expr.line,
                expr.column,
            ));
        }

        let ExprKind::List(bindings) = &items[1].kind else {
            return Err(LispToPythonError::new(
                "`let` の束縛はリストで書いてください",
                items[1].line,
                items[1].column,
            ));
        };

        for binding in bindings {
            let ExprKind::List(pair) = &binding.kind else {
                return Err(LispToPythonError::new(
                    "`let` の束縛は `(name value)` で書いてください",
                    binding.line,
                    binding.column,
                ));
            };
            if pair.len() != 2 {
                return Err(LispToPythonError::new(
                    "`let` の束縛は `(name value)` で書いてください",
                    binding.line,
                    binding.column,
                ));
            }
            let Some(name) = symbol_name(&pair[0]) else {
                return Err(LispToPythonError::new(
                    "`let` の変数名は symbol で書いてください",
                    pair[0].line,
                    pair[0].column,
                ));
            };
            let value = self.emit_expr(&pair[1])?;
            self.line(indent, &format!("{} = {value}", py_identifier(name)));
        }

        self.emit_block(&items[2..], indent, mode)
    }

    fn emit_loop(
        &mut self,
        body: &[Expr],
        indent: usize,
        mode: BodyMode,
    ) -> Result<(), LispToPythonError> {
        self.line(indent, "while True:");
        self.emit_block_or_pass(body, indent + 1, mode)
    }

    fn emit_return(&mut self, items: &[Expr], indent: usize) -> Result<(), LispToPythonError> {
        match items {
            [_] => self.line(indent, "return"),
            [_, value] => {
                let value = self.emit_expr(value)?;
                self.line(indent, &format!("return {value}"));
            }
            [head, ..] => {
                return Err(LispToPythonError::new(
                    "`return` の値は0個または1個です",
                    head.line,
                    head.column,
                ));
            }
            [] => {}
        }
        Ok(())
    }

    fn emit_set(
        &mut self,
        expr: &Expr,
        items: &[Expr],
        indent: usize,
    ) -> Result<(), LispToPythonError> {
        if items.len() != 3 {
            return Err(LispToPythonError::new(
                "`set!` は `(set! name value)` で書いてください",
                expr.line,
                expr.column,
            ));
        }
        let Some(name) = symbol_name(&items[1]) else {
            return Err(LispToPythonError::new(
                "`set!` の変数名は symbol で書いてください",
                items[1].line,
                items[1].column,
            ));
        };
        let value = self.emit_expr(&items[2])?;
        self.line(indent, &format!("{} = {value}", py_identifier(name)));
        Ok(())
    }

    fn emit_set_index(
        &mut self,
        expr: &Expr,
        items: &[Expr],
        indent: usize,
    ) -> Result<(), LispToPythonError> {
        if items.len() != 4 {
            return Err(LispToPythonError::new(
                "`set-index!` は `(set-index! xs i value)` で書いてください",
                expr.line,
                expr.column,
            ));
        }
        let target = self.emit_expr(&items[1])?;
        let index = self.emit_expr(&items[2])?;
        let value = self.emit_expr(&items[3])?;
        self.line(indent, &format!("{target}[{index}] = {value}"));
        Ok(())
    }

    fn emit_while(
        &mut self,
        expr: &Expr,
        items: &[Expr],
        indent: usize,
        mode: BodyMode,
    ) -> Result<(), LispToPythonError> {
        if items.len() < 3 {
            return Err(LispToPythonError::new(
                "`while` は `(while condition body...)` で書いてください",
                expr.line,
                expr.column,
            ));
        }
        let condition = self.emit_expr(&items[1])?;
        self.line(indent, &format!("while {condition}:"));
        self.emit_block_or_pass(&items[2..], indent + 1, mode)
    }

    fn emit_statement_or_begin(
        &mut self,
        expr: &Expr,
        indent: usize,
        mode: BodyMode,
    ) -> Result<(), LispToPythonError> {
        if let ExprKind::List(items) = &expr.kind
            && matches!(items.first().and_then(symbol_name), Some("begin"))
        {
            return self.emit_block_or_pass(&items[1..], indent, mode);
        }
        self.emit_statement(expr, indent, mode)
    }

    fn emit_block(
        &mut self,
        body: &[Expr],
        indent: usize,
        mode: BodyMode,
    ) -> Result<(), LispToPythonError> {
        for form in body {
            self.emit_statement(form, indent, mode)?;
        }
        Ok(())
    }

    fn emit_block_or_pass(
        &mut self,
        body: &[Expr],
        indent: usize,
        mode: BodyMode,
    ) -> Result<(), LispToPythonError> {
        if body.is_empty() {
            self.line(indent, "pass");
            return Ok(());
        }
        self.emit_block(body, indent, mode)
    }

    fn emit_expr(&mut self, expr: &Expr) -> Result<String, LispToPythonError> {
        match &expr.kind {
            ExprKind::Bool(value) => Ok(if *value { "True" } else { "False" }.to_string()),
            ExprKind::Keyword(value) => Ok(keyword_to_python(value)),
            ExprKind::Number(value) => Ok(value.clone()),
            ExprKind::String(value) => Ok(format!("{value:?}")),
            ExprKind::Symbol(value) => Ok(py_identifier(value)),
            ExprKind::List(items) => self.emit_list_expr(expr, items),
        }
    }

    fn emit_list_expr(&mut self, expr: &Expr, items: &[Expr]) -> Result<String, LispToPythonError> {
        if items.is_empty() {
            return Err(LispToPythonError::new(
                "空のリストは式として使えません",
                expr.line,
                expr.column,
            ));
        }

        let Some(head) = symbol_name(&items[0]) else {
            return Err(LispToPythonError::new(
                "関数名は symbol で書いてください",
                items[0].line,
                items[0].column,
            ));
        };

        match head {
            "+" | "*" => self.emit_infix(expr, head, &items[1..]),
            "-" => self.emit_minus(expr, &items[1..]),
            "/" | "<" | "<=" | ">" | ">=" | "!=" => self.emit_infix(expr, head, &items[1..]),
            "=" => self.emit_infix(expr, "==", &items[1..]),
            "and" | "or" => self.emit_infix(expr, head, &items[1..]),
            "dict" | "list" | "set" => self.emit_call(head, &items[1..]),
            "direction" => self.emit_direction_const(expr, &items[1..]),
            "entity" => self.emit_namespace_const(expr, "Entities", &items[1..]),
            "ground" => self.emit_namespace_const(expr, "Grounds", &items[1..]),
            "index" => self.emit_index(expr, &items[1..]),
            "item" => self.emit_namespace_const(expr, "Items", &items[1..]),
            "leaderboard" => self.emit_namespace_const(expr, "Leaderboards", &items[1..]),
            "not" => self.emit_not(expr, &items[1..]),
            "unlock" => self.emit_namespace_const(expr, "Unlocks", &items[1..]),
            _ => self.emit_call(head, &items[1..]),
        }
    }

    fn emit_call(&mut self, name: &str, args: &[Expr]) -> Result<String, LispToPythonError> {
        let py_name = match name {
            "move-dir" => "move".to_string(),
            "quick-print" => "quick_print".to_string(),
            "use-item" => "use_item".to_string(),
            other => py_identifier(other),
        };
        let args = args
            .iter()
            .map(|arg| self.emit_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!("{py_name}({})", args.join(", ")))
    }

    fn emit_direction_const(
        &mut self,
        expr: &Expr,
        args: &[Expr],
    ) -> Result<String, LispToPythonError> {
        if args.len() != 1 {
            return Err(LispToPythonError::new(
                "`direction` は `(direction north)` の形で書いてください",
                expr.line,
                expr.column,
            ));
        }
        const_name(&args[0])
    }

    fn emit_index(&mut self, expr: &Expr, args: &[Expr]) -> Result<String, LispToPythonError> {
        if args.len() != 2 {
            return Err(LispToPythonError::new(
                "`index` は `(index xs i)` の形で書いてください",
                expr.line,
                expr.column,
            ));
        }
        let target = self.emit_expr(&args[0])?;
        let index = self.emit_expr(&args[1])?;
        Ok(format!("{target}[{index}]"))
    }

    fn emit_infix(
        &mut self,
        expr: &Expr,
        op: &str,
        args: &[Expr],
    ) -> Result<String, LispToPythonError> {
        if args.len() < 2 {
            return Err(LispToPythonError::new(
                format!("`{op}` は2個以上の引数が必要です"),
                expr.line,
                expr.column,
            ));
        }
        let args = args
            .iter()
            .map(|arg| self.emit_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!("({})", args.join(&format!(" {op} "))))
    }

    fn emit_minus(&mut self, expr: &Expr, args: &[Expr]) -> Result<String, LispToPythonError> {
        if args.is_empty() {
            return Err(LispToPythonError::new(
                "`-` は1個以上の引数が必要です",
                expr.line,
                expr.column,
            ));
        }
        if args.len() == 1 {
            let value = self.emit_expr(&args[0])?;
            return Ok(format!("(-{value})"));
        }
        self.emit_infix(expr, "-", args)
    }

    fn emit_namespace_const(
        &mut self,
        expr: &Expr,
        namespace: &str,
        args: &[Expr],
    ) -> Result<String, LispToPythonError> {
        if args.len() != 1 {
            return Err(LispToPythonError::new(
                "ゲーム定数は引数を1個だけ取ります",
                expr.line,
                expr.column,
            ));
        }
        Ok(format!("{namespace}.{}", const_name(&args[0])?))
    }

    fn emit_not(&mut self, expr: &Expr, args: &[Expr]) -> Result<String, LispToPythonError> {
        if args.len() != 1 {
            return Err(LispToPythonError::new(
                "`not` は引数を1個だけ取ります",
                expr.line,
                expr.column,
            ));
        }
        let value = self.emit_expr(&args[0])?;
        Ok(format!("not {value}"))
    }

    fn line(&mut self, indent: usize, text: &str) {
        self.output.push_str(&"    ".repeat(indent));
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn blank_line(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }
}

#[derive(Debug)]
struct FunctionDefine {
    name: String,
    params: Vec<String>,
    body: Vec<Expr>,
}

fn parse_function_define(expr: &Expr) -> Result<Option<FunctionDefine>, LispToPythonError> {
    let ExprKind::List(items) = &expr.kind else {
        return Ok(None);
    };
    if !matches!(items.first().and_then(symbol_name), Some("define")) {
        return Ok(None);
    }
    if items.len() < 3 {
        return Ok(None);
    }

    let ExprKind::List(signature) = &items[1].kind else {
        return Ok(None);
    };
    if signature.is_empty() {
        return Err(LispToPythonError::new(
            "`define` の関数名が必要です",
            items[1].line,
            items[1].column,
        ));
    }

    let Some(name) = symbol_name(&signature[0]) else {
        return Err(LispToPythonError::new(
            "`define` の関数名は symbol で書いてください",
            signature[0].line,
            signature[0].column,
        ));
    };
    let mut params = Vec::new();
    for param in &signature[1..] {
        let Some(param) = symbol_name(param) else {
            return Err(LispToPythonError::new(
                "関数引数は symbol で書いてください",
                param.line,
                param.column,
            ));
        };
        params.push(py_identifier(param));
    }

    Ok(Some(FunctionDefine {
        name: name.to_string(),
        params,
        body: items[2..].to_vec(),
    }))
}

fn is_use_form(expr: &Expr) -> bool {
    let ExprKind::List(items) = &expr.kind else {
        return false;
    };
    matches!(items.first().and_then(symbol_name), Some("use"))
}

fn symbol_name(expr: &Expr) -> Option<&str> {
    match &expr.kind {
        ExprKind::Symbol(value) => Some(value),
        _ => None,
    }
}

fn const_name(expr: &Expr) -> Result<String, LispToPythonError> {
    match &expr.kind {
        ExprKind::Keyword(value) | ExprKind::Symbol(value) => Ok(to_const_variant(value)),
        _ => Err(LispToPythonError::new(
            "定数名は symbol または keyword で書いてください",
            expr.line,
            expr.column,
        )),
    }
}

fn keyword_to_python(value: &str) -> String {
    to_const_variant(value)
}

fn py_identifier(value: &str) -> String {
    value.replace('-', "_")
}

fn to_const_variant(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!(
                "{}{}",
                first.to_ascii_uppercase(),
                chars.collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("_")
}

fn trim_extra_blank_lines(value: &str) -> String {
    let mut output = value.trim_end_matches('\n').to_string();
    if !output.is_empty() {
        output.push('\n');
    }
    output
}
