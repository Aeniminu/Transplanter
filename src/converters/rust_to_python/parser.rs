use super::error::RustToPythonError;
use super::ir::{
    Constant, ElseBranch, Expr, Function, FunctionParam, NamespaceAlias, Program, Stmt,
    StructFactory,
};
use super::lexer::{Token, TokenKind};
use super::strict;

pub fn parse(tokens: &[Token]) -> Result<Program, RustToPythonError> {
    Parser { tokens, pos: 0 }.parse_program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn parse_program(&mut self) -> Result<Program, RustToPythonError> {
        let mut constants = Vec::new();
        let mut struct_factories = Vec::new();
        let mut namespace_aliases = Vec::new();
        let mut functions = Vec::new();
        let mut main = None;

        while !self.is_eof() {
            if self.match_use_statement()? {
                continue;
            }

            self.match_visibility()?;

            if self.match_metadata_item(
                &[],
                &mut constants,
                &mut struct_factories,
                &mut namespace_aliases,
                &mut functions,
            )? {
                continue;
            }

            self.reject_unsupported_starter()?;
            self.expect_keyword("fn")?;
            let function = self.parse_function_after_fn()?;

            if function.name == "main" {
                if !function.params.is_empty() {
                    let token = self.previous();
                    return Err(RustToPythonError::new(
                        "`fn main` に引数は指定できません",
                        token.line,
                        token.column,
                    ));
                }
                if main.is_some() {
                    let token = self.previous();
                    return Err(RustToPythonError::new(
                        "`fn main` が重複しています",
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
            return Err(RustToPythonError::new(
                "`fn main` がありません",
                line,
                column,
            ));
        };

        Ok(Program {
            constants,
            struct_factories,
            namespace_aliases,
            functions,
            main,
        })
    }

    fn parse_function_after_fn(&mut self) -> Result<Function, RustToPythonError> {
        let name = self.expect_ident("関数名が必要です")?;

        if self.check_operator("<") {
            self.skip_generics()?;
        }

        self.expect_symbol('(')?;
        let params = self.parse_params()?;
        self.expect_symbol(')')?;

        if self.match_operator("->") {
            self.skip_return_type()?;
        }

        let body = self.parse_block()?;
        Ok(Function { name, params, body })
    }

    fn parse_function_with_name_prefix(
        &mut self,
        namespace: &[String],
    ) -> Result<Function, RustToPythonError> {
        let original_name = self.expect_ident("関数名が必要です")?;
        let name = prefixed_name(namespace, &original_name);

        if self.check_operator("<") {
            self.skip_generics()?;
        }

        self.expect_symbol('(')?;
        let params = self.parse_params()?;
        self.expect_symbol(')')?;

        if self.match_operator("->") {
            self.skip_return_type()?;
        }

        let body = self.parse_block()?;
        Ok(Function { name, params, body })
    }

    fn parse_params(&mut self) -> Result<Vec<FunctionParam>, RustToPythonError> {
        let mut params = Vec::new();

        while !self.check_symbol(')') {
            self.match_operator("&");
            self.match_keyword("mut");
            let name = self.expect_ident("引数名が必要です")?;

            if self.match_symbol(':') {
                self.skip_param_type()?;
            }

            if self.match_operator("=") {
                let token = self.previous();
                return Err(RustToPythonError::new(
                    "デフォルト引数はRustでは使えません。引数を必須にするか、関数内で値を決めてください",
                    token.line,
                    token.column,
                ));
            }

            let default = None;

            params.push(FunctionParam { name, default });

            if !self.match_symbol(',') {
                break;
            }
        }

        Ok(params)
    }

    fn skip_generics(&mut self) -> Result<(), RustToPythonError> {
        self.expect_operator("<")?;
        let mut depth = 1usize;

        while !self.is_eof() {
            if self.match_operator("<") {
                depth += 1;
                continue;
            }
            if self.match_operator(">") {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
                continue;
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(
            "generics の `>` が必要です",
            line,
            column,
        ))
    }

    fn skip_param_type(&mut self) -> Result<(), RustToPythonError> {
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut brace_depth = 0usize;
        let mut angle_depth = 0usize;

        while !self.is_eof() {
            let token = self.peek().expect("not eof");

            match &token.kind {
                TokenKind::Operator(value) if value == "<" => {
                    angle_depth += 1;
                    self.advance();
                }
                TokenKind::Operator(value) if value == ">" && angle_depth > 0 => {
                    angle_depth -= 1;
                    self.advance();
                }
                TokenKind::Symbol('(') => {
                    paren_depth += 1;
                    self.advance();
                }
                TokenKind::Symbol(')') if paren_depth > 0 => {
                    paren_depth -= 1;
                    self.advance();
                }
                TokenKind::Symbol('[') => {
                    bracket_depth += 1;
                    self.advance();
                }
                TokenKind::Symbol(']') if bracket_depth > 0 => {
                    bracket_depth -= 1;
                    self.advance();
                }
                TokenKind::Symbol('{') => {
                    brace_depth += 1;
                    self.advance();
                }
                TokenKind::Symbol('}') if brace_depth > 0 => {
                    brace_depth -= 1;
                    self.advance();
                }
                TokenKind::Operator(value)
                    if value == "="
                        && paren_depth == 0
                        && bracket_depth == 0
                        && brace_depth == 0
                        && angle_depth == 0 =>
                {
                    return Ok(());
                }
                TokenKind::Symbol(',') | TokenKind::Symbol(')')
                    if paren_depth == 0
                        && bracket_depth == 0
                        && brace_depth == 0
                        && angle_depth == 0 =>
                {
                    return Ok(());
                }
                _ => self.advance(),
            }
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(
            "引数リストが閉じられていません",
            line,
            column,
        ))
    }

    fn collect_for_iterable_expr(&mut self) -> Result<Expr, RustToPythonError> {
        let mut tokens = Vec::new();
        let mut depth = ExprDepth::default();

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol('{') if depth.is_top_level() && !tokens.is_empty() => {
                    return finish_expr(tokens);
                }
                TokenKind::Symbol(';') | TokenKind::Symbol('}') if depth.is_top_level() => {
                    return Err(RustToPythonError::new(
                        "for の後に `{` が必要です",
                        token.line,
                        token.column,
                    ));
                }
                TokenKind::Comment(_) => {
                    return Err(RustToPythonError::new(
                        "for の後に `{` が必要です",
                        token.line,
                        token.column,
                    ));
                }
                _ => depth.observe(&token.kind),
            }

            if token.expr_token().is_some() {
                tokens.push(token);
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(
            "for の後に `{` が必要です",
            line,
            column,
        ))
    }

    fn match_metadata_item(
        &mut self,
        namespace: &[String],
        constants: &mut Vec<Constant>,
        struct_factories: &mut Vec<StructFactory>,
        namespace_aliases: &mut Vec<NamespaceAlias>,
        functions: &mut Vec<Function>,
    ) -> Result<bool, RustToPythonError> {
        if self.match_keyword("struct") {
            self.parse_struct_item(namespace, struct_factories, namespace_aliases)?;
            return Ok(true);
        }

        if self.match_keyword("enum") {
            self.parse_enum_item(namespace, constants, namespace_aliases)?;
            return Ok(true);
        }

        if self.match_keyword("mod") {
            self.parse_module_item(
                namespace,
                constants,
                struct_factories,
                namespace_aliases,
                functions,
            )?;
            return Ok(true);
        }

        if self.match_keyword("impl") {
            self.parse_impl_item(namespace, namespace_aliases, functions)?;
            return Ok(true);
        }

        if self.match_keyword("trait") {
            self.skip_item_header_and_block("trait 本体の `{` が必要です")?;
            return Ok(true);
        }

        if self.check_keyword("macro_rules") || self.check_keyword("macro") {
            self.advance();
            self.match_operator("!");
            self.skip_macro_item()?;
            return Ok(true);
        }

        if self.match_keyword("extern") {
            if self.match_keyword("crate") {
                self.skip_statement_until_semicolon("extern crate 文の後に `;` が必要です")?;
                return Ok(true);
            }
            return Err(RustToPythonError::new(
                "`crate` が必要です",
                self.previous().line,
                self.previous().column,
            ));
        }

        Ok(false)
    }

    fn match_local_metadata_item(&mut self) -> Result<bool, RustToPythonError> {
        if self.match_keyword("struct")
            || self.match_keyword("enum")
            || self.match_keyword("mod")
            || self.match_keyword("impl")
            || self.match_keyword("trait")
        {
            self.skip_item_header_and_block("項目本体の `{` が必要です")?;
            return Ok(true);
        }

        if self.check_keyword("macro_rules") || self.check_keyword("macro") {
            self.advance();
            self.match_operator("!");
            self.skip_macro_item()?;
            return Ok(true);
        }

        Ok(false)
    }

    fn parse_struct_item(
        &mut self,
        namespace: &[String],
        struct_factories: &mut Vec<StructFactory>,
        namespace_aliases: &mut Vec<NamespaceAlias>,
    ) -> Result<(), RustToPythonError> {
        let original_name = self.expect_ident("struct 名が必要です")?;
        let name = prefixed_name(namespace, &original_name);
        let mut path = namespace.to_vec();
        path.push(original_name);
        namespace_aliases.push(NamespaceAlias {
            path,
            output: name.clone(),
        });

        if self.check_operator("<") {
            self.skip_generics()?;
        }

        let fields = if self.match_symbol('{') {
            self.parse_named_fields()?
        } else if self.match_symbol('(') {
            self.parse_tuple_fields()?
        } else {
            self.expect_symbol(';')?;
            Vec::new()
        };

        struct_factories.push(StructFactory { name, fields });
        Ok(())
    }

    fn parse_named_fields(&mut self) -> Result<Vec<String>, RustToPythonError> {
        let mut fields = Vec::new();

        while !self.check_symbol('}') {
            if self.is_eof() {
                let (line, column) = self.end_position();
                return Err(RustToPythonError::new(
                    "struct 本体が閉じられていません",
                    line,
                    column,
                ));
            }

            self.match_visibility()?;
            let field = self.expect_ident("field 名が必要です")?;
            fields.push(field);
            self.expect_symbol(':')?;
            self.skip_type_until_field_separator('}')?;

            if !self.match_symbol(',') {
                break;
            }
        }

        self.expect_symbol('}')?;
        Ok(fields)
    }

    fn parse_tuple_fields(&mut self) -> Result<Vec<String>, RustToPythonError> {
        let mut fields = Vec::new();
        let mut index = 0usize;

        while !self.check_symbol(')') {
            if self.is_eof() {
                let (line, column) = self.end_position();
                return Err(RustToPythonError::new(
                    "tuple struct が閉じられていません",
                    line,
                    column,
                ));
            }

            self.match_visibility()?;
            fields.push(format!("field{index}"));
            index += 1;
            self.skip_type_until_field_separator(')')?;

            if !self.match_symbol(',') {
                break;
            }
        }

        self.expect_symbol(')')?;
        self.expect_semicolon("tuple struct の後に `;` が必要です")?;
        Ok(fields)
    }

    fn parse_enum_item(
        &mut self,
        namespace: &[String],
        constants: &mut Vec<Constant>,
        namespace_aliases: &mut Vec<NamespaceAlias>,
    ) -> Result<(), RustToPythonError> {
        let enum_name = self.expect_ident("enum 名が必要です")?;

        if self.check_operator("<") {
            self.skip_generics()?;
        }

        self.expect_symbol('{')?;
        while !self.check_symbol('}') {
            if self.is_eof() {
                let (line, column) = self.end_position();
                return Err(RustToPythonError::new(
                    "enum 本体が閉じられていません",
                    line,
                    column,
                ));
            }

            let variant = self.expect_ident("variant 名が必要です")?;
            let mut path = namespace.to_vec();
            path.push(enum_name.clone());
            path.push(variant.clone());
            let output = path.join("_");
            constants.push(Constant {
                name: output.clone(),
                value: path.join("."),
            });
            namespace_aliases.push(NamespaceAlias { path, output });

            if self.check_symbol('(') {
                self.skip_balanced_symbol('(', ')')?;
            } else if self.check_symbol('{') {
                self.skip_balanced_symbol('{', '}')?;
            }

            if self.match_operator("=") {
                self.skip_until_item_separator('}')?;
            }

            if !self.match_symbol(',') {
                break;
            }
        }

        self.expect_symbol('}')?;
        Ok(())
    }

    fn parse_module_item(
        &mut self,
        namespace: &[String],
        constants: &mut Vec<Constant>,
        struct_factories: &mut Vec<StructFactory>,
        namespace_aliases: &mut Vec<NamespaceAlias>,
        functions: &mut Vec<Function>,
    ) -> Result<(), RustToPythonError> {
        let module_name = self.expect_ident("module 名が必要です")?;
        if self.match_symbol(';') {
            return Ok(());
        }

        self.expect_symbol('{')?;
        let mut child_namespace = namespace.to_vec();
        child_namespace.push(module_name);

        while !self.check_symbol('}') {
            if self.is_eof() {
                let (line, column) = self.end_position();
                return Err(RustToPythonError::new(
                    "module 本体が閉じられていません",
                    line,
                    column,
                ));
            }

            if self.match_use_statement()? {
                continue;
            }

            self.match_visibility()?;
            if self.match_metadata_item(
                &child_namespace,
                constants,
                struct_factories,
                namespace_aliases,
                functions,
            )? {
                continue;
            }

            if self.match_keyword("fn") {
                let original_name = self.peek_ident("関数名が必要です")?;
                let function = self.parse_function_with_name_prefix(&child_namespace)?;
                let mut path = child_namespace.clone();
                path.push(original_name);
                namespace_aliases.push(NamespaceAlias {
                    path,
                    output: function.name.clone(),
                });
                functions.push(function);
                continue;
            }

            self.skip_item_header_and_block("module 内項目の本体が必要です")?;
        }

        self.expect_symbol('}')?;
        Ok(())
    }

    fn parse_impl_item(
        &mut self,
        namespace: &[String],
        namespace_aliases: &mut Vec<NamespaceAlias>,
        functions: &mut Vec<Function>,
    ) -> Result<(), RustToPythonError> {
        if self.check_operator("<") {
            self.skip_generics()?;
        }

        let header = self.collect_tokens_until_block("impl 本体の `{` が必要です")?;
        let target = infer_impl_target(&header).ok_or_else(|| {
            let (line, column) = self.end_position();
            RustToPythonError::new("impl 対象の型名が必要です", line, column)
        })?;

        self.expect_symbol('{')?;
        let mut impl_namespace = namespace.to_vec();
        impl_namespace.push(target);

        while !self.check_symbol('}') {
            if self.is_eof() {
                let (line, column) = self.end_position();
                return Err(RustToPythonError::new(
                    "impl 本体が閉じられていません",
                    line,
                    column,
                ));
            }

            self.match_visibility()?;
            if self.match_keyword("fn") {
                let original_name = self.peek_ident("関数名が必要です")?;
                let function = self.parse_function_with_name_prefix(&impl_namespace)?;
                let mut path = impl_namespace.clone();
                path.push(original_name);
                namespace_aliases.push(NamespaceAlias {
                    path,
                    output: function.name.clone(),
                });
                functions.push(function);
                continue;
            }

            self.skip_item_header_and_block("impl 内項目の本体が必要です")?;
        }

        self.expect_symbol('}')?;
        Ok(())
    }

    fn skip_return_type(&mut self) -> Result<(), RustToPythonError> {
        while !self.check_symbol('{') {
            let Some(token) = self.peek() else {
                let (line, column) = self.end_position();
                return Err(RustToPythonError::new(
                    "関数本体の `{` が必要です",
                    line,
                    column,
                ));
            };

            match &token.kind {
                TokenKind::Symbol(';') | TokenKind::Symbol('}') => {
                    return Err(RustToPythonError::new(
                        "関数本体の `{` が必要です",
                        token.line,
                        token.column,
                    ));
                }
                TokenKind::Comment(_) => {
                    return Err(RustToPythonError::new(
                        "関数本体の `{` が必要です",
                        token.line,
                        token.column,
                    ));
                }
                _ => self.advance(),
            }
        }

        Ok(())
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, RustToPythonError> {
        self.expect_symbol('{')?;
        let mut body = Vec::new();

        while !self.check_symbol('}') {
            if self.is_eof() {
                let (line, column) = self.end_position();
                return Err(RustToPythonError::new(
                    "ブロックが閉じられていません",
                    line,
                    column,
                ));
            }
            body.push(self.parse_stmt()?);
        }

        self.expect_symbol('}')?;
        Ok(body)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, RustToPythonError> {
        self.match_visibility()?;

        if self.match_local_metadata_item()? {
            return Ok(Stmt::Noop);
        }

        self.reject_unsupported_starter()?;

        if let Some(comment) = self.match_comment() {
            return Ok(Stmt::Comment(comment));
        }

        if self.match_keyword("use") {
            self.skip_statement_until_semicolon("use 文の後に `;` が必要です")?;
            return Ok(Stmt::Noop);
        }

        if self.match_keyword("fn") {
            return Ok(Stmt::Function(self.parse_function_after_fn()?));
        }

        if self.match_keyword("loop") {
            return Ok(Stmt::Loop(self.parse_block()?));
        }

        if self.match_keyword("while") {
            let condition =
                self.collect_expr_until_symbol('{', "while 条件の後に `{` が必要です")?;
            self.require_expr(&condition, "while 条件が必要です")?;
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
            self.expect_semicolon("break の後に `;` が必要です")?;
            return Ok(Stmt::Break);
        }

        if self.match_keyword("continue") {
            self.expect_semicolon("continue の後に `;` が必要です")?;
            return Ok(Stmt::Continue);
        }

        if self.match_keyword("return") {
            if self.match_symbol(';') {
                return Ok(Stmt::Return(None));
            }
            let value = self.collect_expr_until_semicolon("return 値の後に `;` が必要です")?;
            return Ok(Stmt::Return(Some(value)));
        }

        self.parse_expr_or_assignment_stmt()
    }

    fn parse_if_after_keyword(&mut self) -> Result<Stmt, RustToPythonError> {
        let condition = self.collect_expr_until_symbol('{', "if 条件の後に `{` が必要です")?;
        self.require_expr(&condition, "if 条件が必要です")?;
        let then_body = self.parse_block()?;
        let else_branch = self.parse_else_branch()?;
        Ok(Stmt::If {
            condition,
            then_body,
            else_branch,
        })
    }

    fn parse_else_branch(&mut self) -> Result<Option<ElseBranch>, RustToPythonError> {
        if !self.match_keyword("else") {
            return Ok(None);
        }

        if self.match_keyword("if") {
            let condition =
                self.collect_expr_until_symbol('{', "else if 条件の後に `{` が必要です")?;
            self.require_expr(&condition, "else if 条件が必要です")?;
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

    fn parse_for_after_keyword(&mut self) -> Result<Stmt, RustToPythonError> {
        let variable = self.expect_ident("ループ変数が必要です")?;
        self.expect_keyword("in")?;

        let iterable = self.collect_for_iterable_expr()?;
        self.require_expr(&iterable, "for の対象が必要です")?;
        let body = self.parse_block()?;

        if let Some((start, end)) = split_top_level_range(&iterable) {
            self.require_expr(&start, "for 範囲の開始値が必要です")?;
            self.require_expr(&end, "for 範囲の終了値が必要です")?;
            Ok(Stmt::For {
                variable,
                start,
                end,
                body,
            })
        } else {
            Ok(Stmt::ForEach {
                variable,
                iterable,
                body,
            })
        }
    }

    fn parse_let_after_keyword(&mut self) -> Result<Stmt, RustToPythonError> {
        self.match_keyword("mut");
        let name = self.expect_ident("let の後に変数名が必要です")?;

        if self.match_symbol(':') {
            self.skip_let_type()?;
        }

        if !self.match_operator("=") {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            return Err(RustToPythonError::new(
                "let 文には `=` が必要です",
                line,
                column,
            ));
        }

        let value = self.collect_expr_until_semicolon("let 文の後に `;` が必要です")?;
        self.require_expr(&value, "let 文には値が必要です")?;
        Ok(Stmt::Let { name, value })
    }

    fn skip_let_type(&mut self) -> Result<(), RustToPythonError> {
        let mut depth = ExprDepth::default();
        let mut angle_depth = 0usize;

        while !self.is_eof() {
            let token = self.peek().expect("not eof");
            match &token.kind {
                TokenKind::Operator(value) if value == "<" => {
                    angle_depth += 1;
                    self.advance();
                }
                TokenKind::Operator(value) if value == ">" && angle_depth > 0 => {
                    angle_depth -= 1;
                    self.advance();
                }
                TokenKind::Operator(value)
                    if value == "=" && depth.is_top_level() && angle_depth == 0 =>
                {
                    return Ok(());
                }
                TokenKind::Symbol(';') | TokenKind::Symbol('}') if depth.is_top_level() => {
                    return Err(RustToPythonError::new(
                        "let 文には `=` が必要です",
                        token.line,
                        token.column,
                    ));
                }
                _ => {
                    depth.observe(&token.kind);
                    self.advance();
                }
            }
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(
            "let 文には `=` が必要です",
            line,
            column,
        ))
    }

    fn parse_expr_or_assignment_stmt(&mut self) -> Result<Stmt, RustToPythonError> {
        let expr = self.collect_expr_until_semicolon("式文の後に `;` が必要です")?;
        self.require_expr(&expr, "式が必要です")?;
        let Some(index) = top_level_assignment_index(&expr) else {
            return Ok(Stmt::Expr(expr));
        };

        let target = Expr::new(expr.tokens[..index].to_vec());
        let value = Expr::new(expr.tokens[index + 1..].to_vec());

        if target.is_empty() || value.is_empty() {
            let token = self.previous();
            return Err(RustToPythonError::new(
                "代入文が不正です",
                token.line,
                token.column,
            ));
        }

        Ok(Stmt::Assign { target, value })
    }

    fn collect_expr_until_semicolon(
        &mut self,
        missing_message: &str,
    ) -> Result<Expr, RustToPythonError> {
        let mut tokens = Vec::new();
        let mut depth = ExprDepth::default();

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol(';') if depth.is_top_level() => {
                    self.advance();
                    return finish_expr(tokens);
                }
                TokenKind::Symbol('}') if depth.is_top_level() => {
                    return Err(RustToPythonError::new(
                        missing_message,
                        token.line,
                        token.column,
                    ));
                }
                TokenKind::Comment(_) => {
                    return Err(RustToPythonError::new(
                        missing_message,
                        token.line,
                        token.column,
                    ));
                }
                _ => depth.observe(&token.kind),
            }

            if token.expr_token().is_some() {
                tokens.push(token);
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(missing_message, line, column))
    }

    fn collect_expr_until_symbol(
        &mut self,
        symbol: char,
        missing_message: &str,
    ) -> Result<Expr, RustToPythonError> {
        let mut tokens = Vec::new();
        let mut depth = ExprDepth::default();

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol(value) if *value == symbol && depth.is_top_level() => {
                    return finish_expr(tokens);
                }
                TokenKind::Symbol(';') | TokenKind::Symbol('}') if depth.is_top_level() => {
                    return Err(RustToPythonError::new(
                        missing_message,
                        token.line,
                        token.column,
                    ));
                }
                TokenKind::Comment(_) => {
                    return Err(RustToPythonError::new(
                        missing_message,
                        token.line,
                        token.column,
                    ));
                }
                _ => depth.observe(&token.kind),
            }

            if token.expr_token().is_some() {
                tokens.push(token);
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(missing_message, line, column))
    }

    fn require_expr(&self, expr: &Expr, message: &str) -> Result<(), RustToPythonError> {
        if !expr.is_empty() {
            return Ok(());
        }

        let token = self.peek().or_else(|| self.tokens.last());
        let (line, column) = token
            .map(|t| (t.line, t.column))
            .unwrap_or_else(|| self.end_position());
        Err(RustToPythonError::new(message, line, column))
    }

    fn reject_unsupported_starter(&self) -> Result<(), RustToPythonError> {
        Ok(())
    }

    fn match_use_statement(&mut self) -> Result<bool, RustToPythonError> {
        if !self.check_keyword("use") {
            return Ok(false);
        }

        if self.check_prelude_import() {
            self.pos += 7;
            return Ok(true);
        }

        self.advance();
        self.skip_statement_until_semicolon("use 文の後に `;` が必要です")?;
        Ok(true)
    }

    fn check_prelude_import(&self) -> bool {
        matches!(
            (
                self.tokens.get(self.pos).map(|token| &token.kind),
                self.tokens.get(self.pos + 1).map(|token| &token.kind),
                self.tokens.get(self.pos + 2).map(|token| &token.kind),
                self.tokens.get(self.pos + 3).map(|token| &token.kind),
                self.tokens.get(self.pos + 4).map(|token| &token.kind),
                self.tokens.get(self.pos + 5).map(|token| &token.kind),
                self.tokens.get(self.pos + 6).map(|token| &token.kind),
            ),
            (
                Some(TokenKind::Ident(use_keyword)),
                Some(TokenKind::Ident(crate_name)),
                Some(TokenKind::Operator(first_colons)),
                Some(TokenKind::Ident(module_name)),
                Some(TokenKind::Operator(second_colons)),
                Some(TokenKind::Operator(glob)),
                Some(TokenKind::Symbol(';')),
            ) if use_keyword == "use"
                && crate_name == "transplanter_rust"
                && first_colons == "::"
                && module_name == "prelude"
                && second_colons == "::"
                && glob == "*"
        )
    }

    fn skip_statement_until_semicolon(
        &mut self,
        missing_message: &str,
    ) -> Result<(), RustToPythonError> {
        let mut depth = ExprDepth::default();

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol(';') if depth.is_top_level() => {
                    self.advance();
                    return Ok(());
                }
                TokenKind::Symbol('}') if depth.is_top_level() => {
                    return Err(RustToPythonError::new(
                        missing_message,
                        token.line,
                        token.column,
                    ));
                }
                _ => depth.observe(&token.kind),
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(missing_message, line, column))
    }

    fn match_visibility(&mut self) -> Result<bool, RustToPythonError> {
        if !self.match_keyword("pub") {
            return Ok(false);
        }

        if self.check_symbol('(') {
            self.skip_balanced_symbol('(', ')')?;
        }

        Ok(true)
    }

    fn skip_type_until_field_separator(&mut self, close: char) -> Result<(), RustToPythonError> {
        let mut depth = ExprDepth::default();

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol(',') if depth.is_top_level() => return Ok(()),
                TokenKind::Symbol(value) if *value == close && depth.is_top_level() => {
                    return Ok(());
                }
                TokenKind::Comment(_) => {
                    return Err(RustToPythonError::new(
                        "型注釈が閉じられていません",
                        token.line,
                        token.column,
                    ));
                }
                _ => depth.observe(&token.kind),
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(
            "型注釈が閉じられていません",
            line,
            column,
        ))
    }

    fn skip_until_item_separator(&mut self, close: char) -> Result<(), RustToPythonError> {
        let mut depth = ExprDepth::default();

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol(',') if depth.is_top_level() => return Ok(()),
                TokenKind::Symbol(value) if *value == close && depth.is_top_level() => {
                    return Ok(());
                }
                _ => depth.observe(&token.kind),
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(
            "項目が閉じられていません",
            line,
            column,
        ))
    }

    fn collect_tokens_until_block(
        &mut self,
        missing_message: &str,
    ) -> Result<Vec<TokenKind>, RustToPythonError> {
        let mut tokens = Vec::new();
        let mut depth = ExprDepth::default();

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol('{') if depth.is_top_level() => return Ok(tokens),
                TokenKind::Symbol(';') | TokenKind::Symbol('}') if depth.is_top_level() => {
                    return Err(RustToPythonError::new(
                        missing_message,
                        token.line,
                        token.column,
                    ));
                }
                _ => depth.observe(&token.kind),
            }
            tokens.push(token.kind);
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(missing_message, line, column))
    }

    fn skip_item_header_and_block(
        &mut self,
        missing_message: &str,
    ) -> Result<(), RustToPythonError> {
        let mut depth = ExprDepth::default();

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match &token.kind {
                TokenKind::Symbol(';') if depth.is_top_level() => {
                    self.advance();
                    return Ok(());
                }
                TokenKind::Symbol('{') if depth.is_top_level() => {
                    self.skip_balanced_symbol('{', '}')?;
                    return Ok(());
                }
                TokenKind::Symbol('}') if depth.is_top_level() => {
                    return Err(RustToPythonError::new(
                        missing_message,
                        token.line,
                        token.column,
                    ));
                }
                _ => depth.observe(&token.kind),
            }
            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(missing_message, line, column))
    }

    fn skip_macro_item(&mut self) -> Result<(), RustToPythonError> {
        while !self.is_eof() {
            if matches!(
                self.peek().map(|token| &token.kind),
                Some(TokenKind::Symbol('{'))
                    | Some(TokenKind::Symbol('('))
                    | Some(TokenKind::Symbol('['))
            ) {
                let (open, close) = match self.peek().expect("checked token exists").kind {
                    TokenKind::Symbol('{') => ('{', '}'),
                    TokenKind::Symbol('(') => ('(', ')'),
                    TokenKind::Symbol('[') => ('[', ']'),
                    _ => unreachable!(),
                };
                self.skip_balanced_symbol(open, close)?;
                self.match_symbol(';');
                return Ok(());
            }

            if self.match_symbol(';') {
                return Ok(());
            }

            self.advance();
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(
            "macro 定義が閉じられていません",
            line,
            column,
        ))
    }

    fn skip_balanced_symbol(&mut self, open: char, close: char) -> Result<(), RustToPythonError> {
        self.expect_symbol(open)?;
        let mut depth = 1usize;

        while !self.is_eof() {
            let token = self.peek().expect("not eof").clone();
            match token.kind {
                TokenKind::Symbol(value) if value == open => {
                    depth += 1;
                    self.advance();
                }
                TokenKind::Symbol(value) if value == close => {
                    depth -= 1;
                    self.advance();
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => self.advance(),
            }
        }

        let (line, column) = self.end_position();
        Err(RustToPythonError::new(
            format!("`{close}` が必要です"),
            line,
            column,
        ))
    }

    fn peek_ident(&self, message: &str) -> Result<String, RustToPythonError> {
        let Some(token) = self.peek() else {
            let (line, column) = self.end_position();
            return Err(RustToPythonError::new(message, line, column));
        };

        if let TokenKind::Ident(value) = &token.kind {
            Ok(value.clone())
        } else {
            Err(RustToPythonError::new(message, token.line, token.column))
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), RustToPythonError> {
        if self.match_keyword(keyword) {
            Ok(())
        } else {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            Err(RustToPythonError::new(
                format!("`{keyword}` が必要です"),
                line,
                column,
            ))
        }
    }

    fn expect_ident(&mut self, message: &str) -> Result<String, RustToPythonError> {
        let Some(token) = self.peek() else {
            let (line, column) = self.end_position();
            return Err(RustToPythonError::new(message, line, column));
        };

        if let TokenKind::Ident(value) = &token.kind {
            let value = value.clone();
            self.advance();
            Ok(value)
        } else {
            Err(RustToPythonError::new(message, token.line, token.column))
        }
    }

    fn expect_symbol(&mut self, symbol: char) -> Result<(), RustToPythonError> {
        if self.match_symbol(symbol) {
            Ok(())
        } else {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            Err(RustToPythonError::new(
                format!("`{symbol}` が必要です"),
                line,
                column,
            ))
        }
    }

    fn expect_operator(&mut self, operator: &str) -> Result<(), RustToPythonError> {
        if self.match_operator(operator) {
            Ok(())
        } else {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            Err(RustToPythonError::new(
                format!("`{operator}` が必要です"),
                line,
                column,
            ))
        }
    }

    fn expect_semicolon(&mut self, message: &str) -> Result<(), RustToPythonError> {
        if self.match_symbol(';') {
            Ok(())
        } else {
            let token = self.peek().or_else(|| self.tokens.last());
            let (line, column) = token
                .map(|t| (t.line, t.column))
                .unwrap_or_else(|| self.end_position());
            Err(RustToPythonError::new(message, line, column))
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
        let token = self.peek()?;

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
    top_level_operator_index(expr, "=")
}

fn prefixed_name(namespace: &[String], name: &str) -> String {
    if namespace.is_empty() {
        return name.to_string();
    }

    let mut parts = namespace.to_vec();
    parts.push(name.to_string());
    parts.join("_")
}

fn infer_impl_target(tokens: &[TokenKind]) -> Option<String> {
    let for_index = tokens
        .iter()
        .position(|token| matches!(token, TokenKind::Ident(value) if value == "for"));

    let search = if let Some(index) = for_index {
        &tokens[index + 1..]
    } else {
        tokens
    };

    search.iter().find_map(|token| match token {
        TokenKind::Ident(value) if value != "for" => Some(value.clone()),
        _ => None,
    })
}

fn finish_expr(tokens: Vec<Token>) -> Result<Expr, RustToPythonError> {
    strict::validate_expr(&tokens)?;
    Ok(Expr::new(
        tokens
            .into_iter()
            .filter_map(|token| token.expr_token())
            .collect(),
    ))
}

fn split_top_level_range(expr: &Expr) -> Option<(Expr, Expr)> {
    let index = top_level_operator_index(expr, "..")?;
    Some((
        Expr::new(expr.tokens[..index].to_vec()),
        Expr::new(expr.tokens[index + 1..].to_vec()),
    ))
}

fn top_level_operator_index(expr: &Expr, target: &str) -> Option<usize> {
    let mut depth = ExprDepth::default();
    for (index, token) in expr.tokens.iter().enumerate() {
        match token {
            super::ir::ExprToken::Operator(op) if op == target && depth.is_top_level() => {
                return Some(index);
            }
            _ => depth.observe_expr(token),
        }
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

    fn observe(&mut self, token: &TokenKind) {
        match token {
            TokenKind::Symbol('(') => self.paren += 1,
            TokenKind::Symbol(')') => self.paren = self.paren.saturating_sub(1),
            TokenKind::Symbol('[') => self.bracket += 1,
            TokenKind::Symbol(']') => self.bracket = self.bracket.saturating_sub(1),
            TokenKind::Symbol('{') => self.brace += 1,
            TokenKind::Symbol('}') => self.brace = self.brace.saturating_sub(1),
            _ => {}
        }
    }

    fn observe_expr(&mut self, token: &super::ir::ExprToken) {
        match token {
            super::ir::ExprToken::Symbol('(') => self.paren += 1,
            super::ir::ExprToken::Symbol(')') => self.paren = self.paren.saturating_sub(1),
            super::ir::ExprToken::Symbol('[') => self.bracket += 1,
            super::ir::ExprToken::Symbol(']') => self.bracket = self.bracket.saturating_sub(1),
            super::ir::ExprToken::Symbol('{') => self.brace += 1,
            super::ir::ExprToken::Symbol('}') => self.brace = self.brace.saturating_sub(1),
            _ => {}
        }
    }
}
