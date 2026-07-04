// Arrow function parsing: predicate scans (`x =>`, `(...) =>`, `<T>() =>`) and
// the builders for parenthesized, single-param, generic, and async arrows. The
// Pratt kernel in `expression.rs` calls into these; they never call back into it.

use crate::ast::internal::{ArrowFunctionBody, ArrowFunctionExpression, Expression, Identifier};
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::Parser;
use super::expression_lookahead::{
    scan_angle_brackets, scan_identifier_then_arrow, scan_parens_then_arrow,
};
use super::scan::skip_whitespace_and_comments;

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Consume the arrow `=>`, enforcing `ArrowFunction`'s `[no LineTerminator here]`
    /// restriction (ecma262): no line terminator between the arrow parameters (or
    /// return type) and `=>`. A bare newline, a line comment, or a block comment
    /// containing or followed by a newline in that gap is a syntax error — acorn
    /// rejects it (the cover grammar reinterprets the `(…)` as a parenthesized
    /// expression, leaving `=>` unexpected), and as a drop-in replacement tsv must
    /// too. The byte-scan arrow predicates skip newlines, so this is the single
    /// enforcement point, shared by the paren, single-param, generic, and async
    /// arrow builders. A same-line block comment (`(a) /* c */ =>`) carries no line
    /// terminator and stays valid.
    fn expect_arrow(&mut self) -> Result<(), ParseError> {
        if self.check(&TokenKind::Arrow) && self.had_line_terminator {
            return Err(self.error_msg("Line terminator not permitted before '=>'"));
        }
        self.expect(&TokenKind::Arrow)
    }

    /// Check if current position starts an arrow function
    ///
    /// Scans ahead looking for pattern: `(` ... `)` `=>`
    pub(super) fn is_arrow_function_start(&self) -> bool {
        scan_parens_then_arrow(self.source.as_bytes(), self.current.start as usize)
    }

    /// Check if current position starts a single-param arrow function: `x =>`
    ///
    /// Scans ahead looking for pattern: `identifier` `=>`
    pub(super) fn is_single_param_arrow_start(&self) -> bool {
        scan_identifier_then_arrow(self.source.as_bytes(), self.current.start as usize)
    }

    /// Check if current position starts a generic arrow function: `<T>() =>`
    ///
    /// Scans ahead looking for pattern: `<` ... `>` `(` ... `)` `=>`
    pub(super) fn is_generic_arrow_function_start(&self) -> bool {
        let bytes = self.source.as_bytes();
        let start = self.current.start as usize;

        // Must start with '<'
        if start >= bytes.len() || bytes[start] != b'<' {
            return false;
        }

        // Scan through type parameters: <T, U extends V, ...>
        let pos = scan_angle_brackets(bytes, start);
        if pos == 0 {
            return false;
        }

        // After '>', check for `(...) =>` (allow comments: `<T> /* comment */ () =>`)
        let pos = skip_whitespace_and_comments(bytes, pos);
        scan_parens_then_arrow(bytes, pos)
    }

    /// Parse generic arrow function: `<T>() => ...`, `<T, U extends V>() => ...`
    pub(super) fn parse_generic_arrow_function(
        &mut self,
    ) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Parse type parameters: <T, U extends V, ...>
        let type_parameters = self.parse_type_parameters()?;

        // Capture paren position before parsing params
        let (params_start, _) = self.current_pos();

        // Params + body in the arrow's `[~Await]` context (a non-async arrow is
        // `[~Await]`; the return type between them is await-free).
        let (params, return_type, body) = self.with_in_await(false, |p| {
            let params = p.parse_parameter_list()?.into_bump_slice();
            // Return type annotation: <T>(): type => ... or type predicate
            let return_type = p.parse_optional_return_type()?;
            p.expect_arrow()?; // consume '=>' (no LineTerminator before it)
            let body = p.parse_arrow_body()?;
            Ok((params, return_type, body))
        })?;
        let end = self.prev_token_end() as u32;

        Ok(Expression::ArrowFunctionExpression(
            ArrowFunctionExpression {
                type_parameters: Some(type_parameters),
                params,
                body,
                return_type,
                r#async: false,
                params_start: Some(params_start as u32),
                span: Span::new(start as u32, end),
            },
        ))
    }

    /// Parse arrow function body: expression or block statement
    fn parse_arrow_body(&mut self) -> Result<ArrowFunctionBody<'arena>, ParseError> {
        if self.check(&TokenKind::BraceOpen) {
            // A block body is a `FunctionBody` (`[+In]`) — `in` is the binary
            // operator even when this arrow sits in a for-header init.
            let block = self.with_allow_in(Self::parse_function_body)?;
            Ok(ArrowFunctionBody::BlockStatement(block))
        } else {
            // A concise body is `AssignmentExpression[?In]` — it inherits the
            // outer In context, so `for (a = () => x in y;;)` still rejects.
            // Use assignment_expression so comma doesn't consume next object property
            Ok(ArrowFunctionBody::Expression(
                self.parse_assignment_expression_ref()?,
            ))
        }
    }

    /// Parse arrow function with parentheses: `() => expr` or `(x, y) => expr` or `() => { ... }`
    ///
    /// Supports:
    /// - No parameters: `() => expr`
    /// - Single parameter: `(x) => expr`
    /// - Multiple parameters: `(x, y) => expr`
    /// - Destructuring parameters: `([a, b]) => ...`, `({x, y}) => ...`
    /// - Default values: `(a = 1) => ...`
    /// - Expression body: `() => expr`
    /// - Block body: `() => { ... }`
    ///
    /// Note: Single parameter without parens (`x => expr`) is handled by
    /// `parse_single_param_arrow_function()`.
    pub(super) fn parse_arrow_function(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Capture paren position before parsing params
        let (params_start, _) = self.current_pos();

        // Params + body in the arrow's `[~Await]` context (non-async).
        let (params, return_type, body) = self.with_in_await(false, |p| {
            let params = p.parse_parameter_list()?.into_bump_slice();
            // Return type annotation: (): type => ... or type predicate
            let return_type = p.parse_optional_return_type()?;
            p.expect_arrow()?; // consume '=>' (no LineTerminator before it)
            let body = p.parse_arrow_body()?;
            Ok((params, return_type, body))
        })?;
        let end = self.prev_token_end() as u32;

        Ok(Expression::ArrowFunctionExpression(
            ArrowFunctionExpression {
                type_parameters: None, // Generic arrows like <T>() => {} are handled by parse_generic_arrow_function()
                params,
                body,
                return_type,
                r#async: false, // Non-async arrow function; async ones are parsed via parse_async_arrow_function
                params_start: Some(params_start as u32),
                span: Span::new(start as u32, end),
            },
        ))
    }

    /// Parse single-parameter arrow function without parentheses: `x => expr`
    pub(super) fn parse_single_param_arrow_function(
        &mut self,
    ) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Parse the single parameter: a plain identifier, or `await` as a
        // `BindingIdentifier` at Script `[~Await]` (`await => …`).
        debug_assert!(
            matches!(self.current_kind(), TokenKind::Identifier) || self.at_await_identifier()
        );
        let (id_start, id_end) = self.current_pos();
        let name = self.current_ident_name_or_await();
        self.advance()?;

        let mut params = self.bvec();
        params.push(Expression::Identifier(Identifier::simple(
            name,
            Span::new(id_start as u32, id_end as u32),
        )));
        let params = params.into_bump_slice();

        self.expect_arrow()?; // consume '=>' (no LineTerminator before it)

        // Non-async single-param arrow body is `[~Await]`.
        let body = self.with_in_await(false, Self::parse_arrow_body)?;
        let end = self.prev_token_end() as u32;

        Ok(Expression::ArrowFunctionExpression(
            ArrowFunctionExpression {
                type_parameters: None,
                params,
                body,
                return_type: None, // Single-param without parens can't have return type
                r#async: false,
                params_start: None, // No parens for single-param arrows
                span: Span::new(start as u32, end),
            },
        ))
    }

    /// Parse async arrow function after 'async' has been consumed: `() => ...`, `x => ...`, or `<T>() => ...`
    pub(super) fn parse_async_arrow_function_after_async(
        &mut self,
        start: usize,
    ) -> Result<Expression<'arena>, ParseError> {
        // Check for type parameters: `async <T>() => ...`
        let type_parameters = self.parse_optional_type_parameters()?;

        // Parse parameter list or single parameter
        // Note: with type parameters, must have parentheses
        let (params, params_start): (&'arena [Expression<'arena>], Option<u32>) = if self
            .check(&TokenKind::ParenOpen)
        {
            let (paren_pos, _) = self.current_pos();
            (
                // Async arrow params are `[+Await]`.
                self.with_in_await(true, Self::parse_parameter_list)?
                    .into_bump_slice(),
                Some(paren_pos as u32),
            )
        } else if type_parameters.is_none() && matches!(self.current_kind(), TokenKind::Identifier)
        {
            // Single parameter without parens: `async x => ...`
            // (Not allowed with type parameters)
            let (id_start, id_end) = self.current_pos();
            let name = self.current_ident_name();
            self.advance()?;
            let mut params = self.bvec();
            params.push(Expression::Identifier(Identifier::simple(
                name,
                Span::new(id_start as u32, id_end as u32),
            )));
            (params.into_bump_slice(), None)
        } else {
            return Err(self.error_expected_after("'(' or identifier", "async"));
        };

        // Check for return type annotation or type predicate
        let return_type = self.parse_optional_return_type()?;

        self.expect_arrow()?; // consume '=>' (no LineTerminator before it)

        // Async arrow body is `[+Await]`.
        let body = self.with_in_await(true, Self::parse_arrow_body)?;
        let end = self.prev_token_end() as u32;

        Ok(Expression::ArrowFunctionExpression(
            ArrowFunctionExpression {
                type_parameters,
                params,
                body,
                return_type,
                r#async: true,
                params_start,
                span: Span::new(start as u32, end),
            },
        ))
    }
}
