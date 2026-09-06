// Arrow function parsing: predicate scans (`x =>`, `(...) =>`, `<T>() =>`) and
// the builders for parenthesized, single-param, generic, and async arrows. The
// Pratt kernel in `expression.rs` calls into these; they never call back into it.

use crate::ast::internal::{ArrowFunctionBody, ArrowFunctionExpression, Expression, Identifier};
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::Parser;
use super::expression::ParsedExpr;
use super::expression_lookahead::{
    paren_head_return_colon, scan_angle_brackets, scan_arrow_after_identifier,
    scan_parens_then_arrow,
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
    /// Consume the arrow's `=>` and return the byte offset of its `=` (the arrow
    /// token start), so builders can record it on the node instead of the printer
    /// re-scanning source for it.
    fn expect_arrow(&mut self) -> Result<u32, ParseError> {
        if self.check(&TokenKind::Arrow) && self.had_line_terminator {
            return Err(self.error_msg("Line terminator not permitted before '=>'"));
        }
        // Full-document coordinate (`current_pos` adds `base_offset`), matching the
        // AST spans the printer indexes with — NOT the raw `self.current.start`, which
        // is slice-relative under Svelte embedding. Same reason `params_start` does this.
        let arrow_token = self.current_pos().0 as u32;
        self.expect(&TokenKind::Arrow)?;
        Ok(arrow_token)
    }

    /// Parse the arrow function whose head a byte-scan predicate just matched, under
    /// tsc's one context rule about its return type — the
    /// `allowReturnTypeInArrowFunction` check in
    /// `parseParenthesizedArrowFunctionExpression`.
    ///
    /// In a conditional's consequent the `:` after a parenthesized head may be the
    /// conditional's own: `a ? (b) : c => d` is `a ? b : (c => d)`, not an arrow
    /// `(b): c => d` with nothing left to end the conditional. tsc parses the arrow
    /// anyway and keeps it only when **another `:` follows** it (`a ? (b): c => d : e`
    /// — a syntax error read any other way, since JavaScript has no second colon
    /// there); otherwise it rewinds and reads the head as a parenthesized expression.
    /// The same here: while the return type is barred
    /// ([`Parser::arrow_return_type_barred`]) the parse runs from a
    /// [`Parser::checkpoint`], and an annotated arrow not followed by `:` is rewound
    /// — `None` hands the head back for the caller to read as what it otherwise is
    /// (a grouped expression, a type assertion, a call to `async`). Outside a
    /// consequent, and for an arrow without an annotation, `parse` is the whole
    /// story. The bar reaches only heads at the consequent's own grouping depth;
    /// inside any delimiter opened since the `?`, and in a `[+In]` body or a
    /// `yield` argument, the annotation is always allowed, as tsc passes `true`
    /// there.
    ///
    /// Only an **annotated** head is ever in question (tsc's `hasReturnColon` gate),
    /// and the annotation is read off the bytes (`paren_head_return_colon`, from the
    /// head's `(` at `paren`) before the parse rather than off the node after it,
    /// because an annotated head that fails to parse as an arrow is rewound too —
    /// `a ? (b + c) : d => e` is a head tsc never reads as a signature, and its
    /// `tryParse` rewinds where tsv's parameter parse errors. A body error rewinds
    /// the same way, to a verdict tsc shares: the parenthesized reading hands the
    /// same body to the alternate's arrow, which fails on it again. A head without
    /// an annotation parses straight, so a body error there keeps its message.
    ///
    /// tsc additionally commits an *unambiguous* head (`(a: T)`, `()`, `(...a)`,
    /// `(a?: T)`) to the annotation without asking — a distinction this need not
    /// draw: such a head has no parenthesized-expression reading, so its rewound
    /// parse fails and the failure flows outward exactly as tsc's committed arrow
    /// leaves the enclosing conditional without its `:` — an enclosing speculation
    /// rewinds on it and its alternate re-reads the head under the lifted rule
    /// (`a ? (b) : c => (d: D): E => e`), and at the top the error stands.
    ///
    /// `parse` is a plain `fn` pointer rather than a generic closure so the cold
    /// speculation below has ONE body across the three head sites; the fast path
    /// inlines here, where the pointer is a constant and the call stays direct.
    #[inline]
    pub(super) fn parse_arrow_or_rewind(
        &mut self,
        paren: usize,
        parse: fn(&mut Self) -> Result<ParsedExpr<'arena>, ParseError>,
    ) -> Result<Option<ParsedExpr<'arena>>, ParseError> {
        if !self.arrow_return_type_barred()
            || !paren_head_return_colon(self.source.as_bytes(), paren)
        {
            return parse(self).map(Some);
        }
        self.parse_arrow_speculatively(parse)
    }

    /// The speculative half of [`Parser::parse_arrow_or_rewind`]: an annotated head
    /// in a barred context, parsed from a checkpoint and kept only when `:` follows.
    #[cold]
    #[inline(never)]
    fn parse_arrow_speculatively(
        &mut self,
        parse: fn(&mut Self) -> Result<ParsedExpr<'arena>, ParseError>,
    ) -> Result<Option<ParsedExpr<'arena>>, ParseError> {
        let checkpoint = self.checkpoint();
        match parse(self) {
            Ok(arrow) if self.check(&TokenKind::Colon) => Ok(Some(arrow)),
            Ok(_) | Err(_) => {
                self.rewind(checkpoint);
                Ok(None)
            }
        }
    }

    /// Check if current position starts an arrow function
    ///
    /// Scans ahead looking for pattern: `(` ... `)` `=>`. Returns the head's `(`
    /// (slice-relative, `self.current.start`) when it is one — what
    /// `parse_arrow_or_rewind` reads the annotation off, as for the generic head.
    pub(super) fn paren_arrow_function_start(&self) -> Option<usize> {
        let paren = self.current.start as usize;
        scan_parens_then_arrow(self.source.as_bytes(), paren).then_some(paren)
    }

    /// Check if current position starts a single-param arrow function: `x =>`
    ///
    /// Scans ahead looking for pattern: `identifier` `=>`
    pub(super) fn is_single_param_arrow_start(&self) -> bool {
        // The current token IS the identifier, so its end is already known — see
        // `scan_arrow_after_identifier`, which no longer re-walks the name to find it.
        // Slice-relative like `self.source`, matching the byte slice this indexes.
        scan_arrow_after_identifier(self.source.as_bytes(), self.current.end as usize)
    }

    /// Check if current position starts a generic arrow function: `<T>() =>`
    ///
    /// Scans ahead looking for pattern: `<` ... `>` `(` ... `)` `=>`
    ///
    /// Returns the head's `(` (slice-relative, like `self.current.start`) when it
    /// is one — what `parse_arrow_or_rewind` reads the annotation off.
    pub(super) fn generic_arrow_function_start(&self) -> Option<usize> {
        let bytes = self.source.as_bytes();
        let start = self.current.start as usize;

        // Must start with '<'
        if start >= bytes.len() || bytes[start] != b'<' {
            return None;
        }

        // Scan through type parameters: <T, U extends V, ...>
        let pos = scan_angle_brackets(bytes, start);
        if pos == 0 {
            return None;
        }

        // After '>', check for `(...) =>` (allow comments: `<T> /* comment */ () =>`)
        let paren = skip_whitespace_and_comments(bytes, pos);
        scan_parens_then_arrow(bytes, paren).then_some(paren)
    }

    /// Parse generic arrow function: `<T>() => ...`, `<T, U extends V>() => ...`
    pub(super) fn parse_generic_arrow_function(
        &mut self,
    ) -> Result<ParsedExpr<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Parse type parameters: <T, U extends V, ...>
        let type_parameters = self.parse_type_parameters()?;

        // Capture paren position before parsing params
        let (params_start, _) = self.current_pos();

        // Params + body in the arrow's own `[~Await, ~Yield]` context (an arrow is
        // never async/generator itself; the return type between them is await-free,
        // and `yield` in an arrow inside a generator is a plain identifier).
        let (params, return_type, body, arrow_token) = self.with_fn_context(false, false, |p| {
            let params = p.parse_parameter_list_no_decorators()?.into_bump_slice();
            // Return type annotation: <T>(): type => ... or type predicate
            let return_type = p.parse_optional_return_type()?;
            let arrow_token = p.expect_arrow()?; // consume '=>' (no LineTerminator before it)
            let body = p.parse_arrow_body()?;
            Ok((params, return_type, body, arrow_token))
        })?;
        let end = self.prev_token_end() as u32;

        Ok(ParsedExpr::from_expr(
            self.arena,
            Expression::ArrowFunctionExpression(self.arena.alloc(ArrowFunctionExpression {
                type_parameters: Some(type_parameters),
                params,
                body,
                return_type,
                r#async: false,
                params_start: Some(params_start as u32),
                arrow_token,
                span: Span::new(start as u32, end),
            })),
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
    pub(super) fn parse_arrow_function(&mut self) -> Result<ParsedExpr<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Capture paren position before parsing params
        let (params_start, _) = self.current_pos();

        // Params + body in the arrow's own `[~Await, ~Yield]` context (an arrow is
        // never async/generator itself).
        let (params, return_type, body, arrow_token) = self.with_fn_context(false, false, |p| {
            let params = p.parse_parameter_list_no_decorators()?.into_bump_slice();
            // Return type annotation: (): type => ... or type predicate
            let return_type = p.parse_optional_return_type()?;
            let arrow_token = p.expect_arrow()?; // consume '=>' (no LineTerminator before it)
            let body = p.parse_arrow_body()?;
            Ok((params, return_type, body, arrow_token))
        })?;
        let end = self.prev_token_end() as u32;

        Ok(ParsedExpr::from_expr(
            self.arena,
            Expression::ArrowFunctionExpression(self.arena.alloc(ArrowFunctionExpression {
                type_parameters: None, // Generic arrows like <T>() => {} are handled by parse_generic_arrow_function()
                params,
                body,
                return_type,
                r#async: false, // Non-async arrow function; async ones are parsed via parse_async_arrow_function
                params_start: Some(params_start as u32),
                arrow_token,
                span: Span::new(start as u32, end),
            })),
        ))
    }

    /// Parse single-parameter arrow function without parentheses: `x => expr`
    pub(super) fn parse_single_param_arrow_function(
        &mut self,
    ) -> Result<ParsedExpr<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Parse the single parameter: a plain identifier, a contextual type keyword
        // (`any => …`), or `await` as a `BindingIdentifier` at Script `[~Await]`
        // (`await => …`) — the dispatcher gates every reachable case through the
        // `try_binding_name` binding-name predicate.
        debug_assert!(self.at_binding_name());
        let (id_start, id_end) = self.current_pos();
        let name = self.current_ident_name_or_await();
        self.advance()?;

        let mut params = self.bvec();
        params.push(Expression::Identifier(Identifier::simple(
            name,
            Span::new(id_start as u32, id_end as u32),
        )));
        let params = params.into_bump_slice();

        let arrow_token = self.expect_arrow()?; // consume '=>' (no LineTerminator before it)

        // Non-async single-param arrow body is `[~Await]`.
        let body = self.with_fn_context(false, false, Self::parse_arrow_body)?;
        let end = self.prev_token_end() as u32;

        Ok(ParsedExpr::from_expr(
            self.arena,
            Expression::ArrowFunctionExpression(self.arena.alloc(ArrowFunctionExpression {
                type_parameters: None,
                params,
                body,
                return_type: None, // Single-param without parens can't have return type
                r#async: false,
                params_start: None, // No parens for single-param arrows
                arrow_token,
                span: Span::new(start as u32, end),
            })),
        ))
    }

    /// Parse async arrow function after 'async' has been consumed: `() => ...`, `x => ...`, or `<T>() => ...`
    pub(super) fn parse_async_arrow_function_after_async(
        &mut self,
        start: usize,
    ) -> Result<ParsedExpr<'arena>, ParseError> {
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
                self.with_fn_context(true, false, Self::parse_parameter_list_no_decorators)?
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

        let arrow_token = self.expect_arrow()?; // consume '=>' (no LineTerminator before it)

        // Async arrow body is `[+Await]`.
        let body = self.with_fn_context(true, false, Self::parse_arrow_body)?;
        let end = self.prev_token_end() as u32;

        Ok(ParsedExpr::from_expr(
            self.arena,
            Expression::ArrowFunctionExpression(self.arena.alloc(ArrowFunctionExpression {
                type_parameters,
                params,
                body,
                return_type,
                r#async: true,
                params_start,
                arrow_token,
                span: Span::new(start as u32, end),
            })),
        ))
    }
}
