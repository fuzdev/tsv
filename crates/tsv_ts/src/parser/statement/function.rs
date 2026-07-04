// Function declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

impl<'a, 'arena> Parser<'a, 'arena> {
    pub(super) fn parse_return_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'return' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Return)
        ));
        self.advance()?;

        // ASI Rule: If semicolon present or can insert semicolon, return has no argument.
        // This handles: `return\n1+2` → `return;` (then `1+2;` as separate statement)
        // The `1+2` becomes an unreachable expression statement.
        if self.eat(TokenKind::Semicolon) || self.can_insert_semicolon() {
            let end = self.prev_token_end() as u32;
            return Ok(Statement::ReturnStatement(ReturnStatement {
                argument: None,
                span: Span::new(start as u32, end),
            }));
        }

        // No ASI - parse the return value expression
        let argument = self.parse_expression()?;
        let end = self.semicolon_end()?;

        Ok(Statement::ReturnStatement(ReturnStatement {
            argument: Some(argument),
            span: Span::new(start as u32, end),
        }))
    }

    pub(super) fn parse_function_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        self.parse_function_or_overload(false)
    }

    /// Parse function declaration or overload signature
    ///
    /// Function overloads end with semicolon instead of body:
    /// ```typescript
    /// function check(x: unknown): x is string;  // overload - TSDeclareFunction
    /// function check(x: unknown): x is number;  // overload - TSDeclareFunction
    /// function check(x: unknown) { ... }         // implementation - FunctionDeclaration
    /// ```
    fn parse_function_or_overload(
        &mut self,
        is_async: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'function' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Function)
        ));
        self.advance()?;

        // Check for generator: function*
        let is_generator = if matches!(self.current_kind(), TokenKind::Star) {
            self.advance()?;
            true
        } else {
            false
        };

        // Parse function name (required for declarations)
        // Keywords like `object` and `async` can be function names; `await` is a
        // valid name only at Script `[~Await]` (reserved at Module / `[+Await]`).
        let (id_start, id_end) = self.current_pos();
        let Some(name) = self.try_function_name() else {
            return Err(self.error_expected_after("function name", "function"));
        };
        self.advance()?;

        let id = Identifier::simple(name, Span::new(id_start as u32, id_end as u32));

        // Parse type parameters (TypeScript generics): function foo<T>()
        let type_parameters = self.parse_optional_type_parameters()?;

        // Capture paren position before parsing params (for comment detection)
        let (params_start, _) = self.current_pos();

        // Parse parameter list (in the function's own `[Await]` context — async
        // params are `[+Await]`, non-async `[~Await]`).
        let params: &'arena [Expression<'arena>] = self
            .with_in_await(is_async, Self::parse_parameter_list)?
            .into_bump_slice();

        // Check for return type annotation
        let return_type = self.parse_optional_return_type()?;

        // Check if this is an overload (ends with ; or no body) or implementation (has body)
        // Overload signatures don't have a body block - they end with ; or ASI
        if !matches!(self.current_kind(), TokenKind::BraceOpen) {
            // Function overload signature - parse as TSDeclareFunction
            let end = self.semicolon_end()?;

            Ok(Statement::TSDeclareFunction(TSDeclareFunction {
                id,
                type_parameters,
                params,
                return_type,
                declare: false, // Not a `declare function`, just an overload
                r#async: is_async,
                generator: is_generator,
                span: Span::new(start as u32, end),
            }))
        } else {
            // Function implementation - parse body (the function's `[Await]` context).
            let body = self.with_in_await(is_async, Self::parse_function_body)?;
            let end = body.span.end;

            Ok(Statement::FunctionDeclaration(FunctionDeclaration {
                id: Some(id),
                type_parameters,
                params,
                return_type,
                body,
                generator: is_generator,
                r#async: is_async,
                params_start: params_start as u32,
                span: Span::new(start as u32, end),
            }))
        }
    }

    /// Parse async function declaration: `async function foo() {}` or `async function* foo() {}`
    pub(super) fn parse_async_function_declaration(
        &mut self,
    ) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'async' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Async)
        ));
        self.advance()?;

        // Parse the function (which may be an overload signature or implementation)
        let mut stmt = self.parse_function_or_overload(true)?;

        // Update span to include 'async' keyword. `parse_function_or_overload`
        // only ever returns these two variants, both with a `span` field, so an
        // or-pattern patches the start in place without a fallback arm.
        if let Statement::FunctionDeclaration(FunctionDeclaration { span, .. })
        | Statement::TSDeclareFunction(TSDeclareFunction { span, .. }) = &mut stmt
        {
            span.start = start as u32;
        }

        Ok(stmt)
    }

    /// Inner function that can return either FunctionDeclaration or TSDeclareFunction
    ///
    /// Used by export default and export named to handle both regular and ambient contexts
    pub(super) fn parse_function_declaration_or_declare(
        &mut self,
        name_required: bool,
        is_async: bool,
    ) -> Result<ExportFunctionDeclaration<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'function' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Function)
        ));
        self.advance()?;

        // Check for generator: function*
        let is_generator = if matches!(self.current_kind(), TokenKind::Star) {
            self.advance()?;
            true
        } else {
            false
        };

        // Parse function name (required for declarations, optional for export default)
        // Keywords like `object` and `async` can be function names; `await` is a
        // valid name only at Script `[~Await]` (reserved at Module / `[+Await]`).
        let id = if let Some(name) = self.try_function_name() {
            let (id_start, id_end) = self.current_pos();
            self.advance()?;

            Some(Identifier::simple(
                name,
                Span::new(id_start as u32, id_end as u32),
            ))
        } else if name_required {
            return Err(self.error_expected_after("function name", "function"));
        } else {
            None
        };

        // Parse type parameters
        let type_parameters = self.parse_optional_type_parameters()?;

        // Capture paren position before parsing params (for comment detection)
        let (params_start, _) = self.current_pos();

        // Parse parameter list (in the function's own `[Await]` context).
        let params: &'arena [Expression<'arena>] = self
            .with_in_await(is_async, Self::parse_parameter_list)?
            .into_bump_slice();

        // Check for return type annotation
        let return_type = self.parse_optional_return_type()?;

        // Check if this is an overload signature (no body) or implementation (has body)
        if !matches!(self.current_kind(), TokenKind::BraceOpen) {
            // No body - this is a declare function (ambient context)
            let end = self.semicolon_end()?;

            Ok(ExportFunctionDeclaration::Declare(TSDeclareFunction {
                id: id.unwrap_or_else(|| {
                    // Anonymous: empty name over a zero-width span (span-identity).
                    Identifier::simple(
                        IdentName {
                            escaped: None,
                            raw_len: 0,
                        },
                        Span::new(start as u32, start as u32),
                    )
                }),
                type_parameters,
                params,
                return_type,
                declare: false,
                r#async: is_async,
                generator: is_generator,
                span: Span::new(start as u32, end),
            }))
        } else {
            // Has body - regular function declaration (the function's `[Await]` context).
            let body = self.with_in_await(is_async, Self::parse_function_body)?;
            let end = body.span.end;

            Ok(ExportFunctionDeclaration::Declaration(
                FunctionDeclaration {
                    id,
                    type_parameters,
                    params,
                    return_type,
                    body,
                    generator: is_generator,
                    r#async: is_async,
                    params_start: params_start as u32,
                    span: Span::new(start as u32, end),
                },
            ))
        }
    }

    /// Parse a function expression: `function() {}` or `function name<T>() {}`
    ///
    /// Function expressions are similar to function declarations but:
    /// - The name is always optional
    /// - They appear in expression position
    pub fn parse_function_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        self.parse_function_expression_inner(start, false)
    }

    /// Parse an async function expression: `async function() {}` or `async function*() {}`
    ///
    /// Called when we've already seen `async` and are at `function`.
    pub fn parse_async_function_expression(
        &mut self,
        start: usize,
    ) -> Result<Expression<'arena>, ParseError> {
        self.parse_function_expression_inner(start, true)
    }

    /// Core function expression parsing logic
    fn parse_function_expression_inner(
        &mut self,
        start: usize,
        is_async: bool,
    ) -> Result<Expression<'arena>, ParseError> {
        // Consume 'function' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Function)
        ));
        self.advance()?;

        // Check for generator: function*
        let is_generator = if matches!(self.current_kind(), TokenKind::Star) {
            self.advance()?;
            true
        } else {
            false
        };

        // Parse the optional name in the function expression's own `[Await]`
        // context: a `FunctionExpression` name is `[~Await]` (non-async) /
        // `[+Await]` (async), so `await` is a valid name in a non-async function
        // expression even inside a `[+Await]` enclosing scope (e.g. a static
        // block), but not in an async one.
        let id = self.with_in_await(is_async, |p| {
            if matches!(p.current_kind(), TokenKind::Identifier) || p.at_await_identifier() {
                let (id_start, id_end) = p.current_pos();
                let name = p.current_ident_name_or_await();
                p.advance()?;
                Ok(Some(Identifier::simple(
                    name,
                    Span::new(id_start as u32, id_end as u32),
                )))
            } else {
                Ok(None)
            }
        })?;

        // Parse type parameters (TypeScript generics): function<T>()
        let type_parameters = self.parse_optional_type_parameters()?;

        // Capture paren position before parsing params (for comment detection)
        let (params_start, _) = self.current_pos();

        // Param defaults (`[+In]`) and the function body are a fresh `[+In]`
        // context, so `in` is the binary operator even when this function
        // expression sits in a for-header init.
        let (params, return_type, body) = self.with_in_await(is_async, |p| {
            p.with_allow_in(|p| {
                let params: &'arena [Expression<'arena>] =
                    p.parse_parameter_list()?.into_bump_slice();
                let return_type = p.parse_optional_return_type()?;
                let body = p.parse_function_body()?;
                Ok((params, return_type, body))
            })
        })?;
        let end = body.span.end;

        Ok(Expression::FunctionExpression(FunctionExpression {
            id,
            type_parameters,
            params,
            return_type,
            body,
            generator: is_generator,
            r#async: is_async,
            params_start: params_start as u32,
            span: Span::new(start as u32, end),
        }))
    }
}
