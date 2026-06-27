// Control flow statement parsing (if, for, while, switch, try, throw, break, continue, labeled)

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Parse if statement: `if (test) consequent` or `if (test) consequent else alternate`
    pub(super) fn parse_if_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'if' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::If)
        ));
        self.advance()?;

        let arena = self.arena;

        // Parse condition: (test)
        self.expect(&TokenKind::ParenOpen)?;
        let test = self.parse_expression()?;
        self.expect(&TokenKind::ParenClose)?;

        // Parse consequent (can be any statement, including block)
        let consequent = arena.alloc(self.parse_statement()?);

        // Check for optional else clause
        let (alternate, end) =
            if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Else)) {
                self.advance()?; // consume 'else'
                let alt = self.parse_statement()?;
                let alt_end = alt.span().end;
                (Some(&*arena.alloc(alt)), alt_end)
            } else {
                (None, consequent.span().end)
            };

        Ok(Statement::IfStatement(IfStatement {
            test,
            consequent,
            alternate,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse for statement: `for (init; test; update) body` or `for (left in/of right) body`
    /// Also handles `for await (left of right) body`
    pub(super) fn parse_for_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'for' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::For)
        ));
        self.advance()?;

        // Check for 'await' keyword: `for await (...)`
        let is_await = matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Await));
        if is_await {
            self.advance()?;
        }

        self.expect(&TokenKind::ParenOpen)?;

        // Parse init/left part - could be:
        // 1. Empty (for (;;))
        // 2. Variable declaration (for (let x = 0; ...))
        // 3. Expression (for (x = 0; ...))
        // 4. Variable declaration for-in/of (for (let x of ...))
        // 5. Expression pattern for-in/of (for (x of ...))

        if self.eat(TokenKind::Semicolon) {
            // Empty init: for (;...)
            return self.parse_for_standard(start, None);
        }

        // Check if it starts with a variable declaration
        let is_var_decl = matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Const | KeywordKind::Let | KeywordKind::Var)
        );

        // Check for `using` contextual keyword (ES2024 Explicit Resource Management)
        // `for (using resource of resources) { ... }`
        let is_using = *self.current_kind() == TokenKind::Identifier
            && self.current_value() == "using"
            && self.peek_is_identifier();

        // Check for `await using` in for-of
        // `for await (await using resource of resources) { ... }`
        let is_await_using = *self.current_kind() == TokenKind::Keyword(KeywordKind::Await)
            && self.peek_is_identifier()
            && self.peek_value() == "using";

        if is_await_using {
            // Parse `await using` declaration for for-of
            let var_decl = self.parse_for_await_using_declaration()?;

            // `await using` only valid with `of`, not `in` or standard for
            if self.current_value() == "of" {
                self.advance()?;
                return self.parse_for_of(
                    start,
                    ForInOfLeft::VariableDeclaration(var_decl),
                    is_await,
                );
            }

            return Err(self.error_msg("'await using' can only be used in for-of loops"));
        }

        if is_using {
            // Parse `using` declaration for for-of
            let var_decl = self.parse_for_using_declaration()?;

            // `using` only valid with `of`, not `in` or standard for
            if self.current_value() == "of" {
                self.advance()?;
                return self.parse_for_of(
                    start,
                    ForInOfLeft::VariableDeclaration(var_decl),
                    is_await,
                );
            }

            return Err(self.error_msg("'using' can only be used in for-of loops"));
        }

        if is_var_decl {
            // Parse variable declaration (without semicolon)
            let var_decl = self.parse_for_variable_declaration()?;

            // Check for 'in' or 'of'
            if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::In)) {
                self.advance()?;
                return self.parse_for_in(start, ForInOfLeft::VariableDeclaration(var_decl));
            }
            if self.current_value() == "of" {
                self.advance()?;
                return self.parse_for_of(
                    start,
                    ForInOfLeft::VariableDeclaration(var_decl),
                    is_await,
                );
            }

            // Standard for loop with var decl init
            self.expect(&TokenKind::Semicolon)?;
            return self.parse_for_standard(start, Some(ForInit::VariableDeclaration(var_decl)));
        }

        // `for await (async of …)` — here `async` is a plain IdentifierReference
        // LHS, not the start of an `async … =>` arrow (which the generic
        // expression path would assume on seeing `async` followed by `of`). The
        // for-of `[lookahead ∉ { async of }]` restriction applies ONLY to the
        // non-await for-of, so this is gated on `is_await`; plain
        // `for (async of …)` keeps falling through to the normal path and stays
        // rejected (matching acorn).
        if is_await
            && matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Async))
            && self.peek_is_identifier()
            && self.peek_value() == "of"
        {
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern_identifier();
            self.advance()?; // consume 'async'
            let async_ident = Expression::Identifier(Identifier::simple(
                symbol,
                Span::new(id_start as u32, id_end as u32),
            ));
            self.advance()?; // consume 'of'
            return self.parse_for_of(start, ForInOfLeft::Pattern(async_ident), is_await);
        }

        // Parse expression (could be init or left-hand side)
        // Use parse_expression_no_in to prevent `in` from being parsed as binary operator
        let expr = self.parse_expression_no_in()?;

        // Check for 'in' or 'of'
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::In)) {
            self.advance()?;
            return self.parse_for_in(start, ForInOfLeft::Pattern(expr));
        }
        if self.current_value() == "of" {
            self.advance()?;
            return self.parse_for_of(start, ForInOfLeft::Pattern(expr), is_await);
        }

        // Standard for loop with expression init
        self.expect(&TokenKind::Semicolon)?;
        self.parse_for_standard(start, Some(ForInit::Expression(expr)))
    }

    /// Parse standard for loop: `for (init; test; update) body`
    fn parse_for_standard(
        &mut self,
        start: usize,
        init: Option<ForInit<'arena>>,
    ) -> Result<Statement<'arena>, ParseError> {
        // Parse test (optional)
        let test = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&TokenKind::Semicolon)?;

        // Parse update (optional)
        let update = if self.check(&TokenKind::ParenClose) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&TokenKind::ParenClose)?;

        // Parse body
        let body = self.arena.alloc(self.parse_statement()?);
        let end = body.span().end;

        Ok(Statement::ForStatement(ForStatement {
            init,
            test,
            update,
            body,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse for-in loop: `for (left in right) body`
    fn parse_for_in(
        &mut self,
        start: usize,
        left: ForInOfLeft<'arena>,
    ) -> Result<Statement<'arena>, ParseError> {
        let right = self.parse_expression()?;
        self.expect(&TokenKind::ParenClose)?;

        let body = self.arena.alloc(self.parse_statement()?);
        let end = body.span().end;

        Ok(Statement::ForInStatement(ForInStatement {
            left,
            right,
            body,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse for-of loop: `for (left of right) body`
    fn parse_for_of(
        &mut self,
        start: usize,
        left: ForInOfLeft<'arena>,
        r#await: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        let right = self.parse_expression()?;
        self.expect(&TokenKind::ParenClose)?;

        let body = self.arena.alloc(self.parse_statement()?);
        let end = body.span().end;

        Ok(Statement::ForOfStatement(ForOfStatement {
            left,
            right,
            r#await,
            body,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse while statement: `while (test) body`
    pub(super) fn parse_while_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'while' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::While)
        ));
        self.advance()?;

        // Parse condition: (test)
        self.expect(&TokenKind::ParenOpen)?;
        let test = self.parse_expression()?;
        self.expect(&TokenKind::ParenClose)?;

        // Parse body
        let body = self.arena.alloc(self.parse_statement()?);
        let end = body.span().end;

        Ok(Statement::WhileStatement(WhileStatement {
            test,
            body,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse do-while statement: `do body while (test);`
    pub(super) fn parse_do_while_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'do' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Do)
        ));
        self.advance()?;

        // Parse body
        let body = self.arena.alloc(self.parse_statement()?);

        // Expect 'while'
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::While)) {
            return Err(self.error_expected_after("'while'", "do statement body"));
        }
        self.advance()?;

        // Parse condition: (test)
        self.expect(&TokenKind::ParenOpen)?;
        let test = self.parse_expression()?;
        self.expect(&TokenKind::ParenClose)?;

        // A semicolon is automatically inserted after a do-while's `)`
        // *unconditionally* (ASI rule 1, third bullet) — unlike ordinary
        // statement termination it needs no preceding line terminator and no
        // `}`/EOF lookahead, so this never errors. Consume an explicit `;` if
        // present; otherwise insert one implicitly. Local to do-while, so the
        // shared `semicolon()` helper stays restricted.
        self.eat(TokenKind::Semicolon);
        let end = self.prev_token_end() as u32;

        Ok(Statement::DoWhileStatement(DoWhileStatement {
            body,
            test,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse switch statement: `switch (discriminant) { cases }`
    pub(super) fn parse_switch_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'switch' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Switch)
        ));
        self.advance()?;

        // Parse discriminant: (expr)
        self.expect(&TokenKind::ParenOpen)?;
        let discriminant = self.parse_expression()?;
        self.expect(&TokenKind::ParenClose)?;

        // Parse cases: { case ... }
        self.expect(&TokenKind::BraceOpen)?;
        let mut cases = self.bvec();

        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            cases.push(self.parse_switch_case()?);
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(Statement::SwitchStatement(SwitchStatement {
            discriminant,
            cases: cases.into_bump_slice(),
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse switch case: `case test: consequent` or `default: consequent`
    fn parse_switch_case(&mut self) -> Result<SwitchCase<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Check for 'case' or 'default'
        let test = if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Case)) {
            self.advance()?;
            Some(self.parse_expression()?)
        } else if matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Default)
        ) {
            self.advance()?;
            None
        } else {
            return Err(self.error_expected("'case' or 'default'"));
        };

        self.expect(&TokenKind::Colon)?;
        let colon_end = self.prev_token_end();

        // Parse consequent statements until next case/default or closing brace
        let mut consequent = self.bvec();
        while !matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Case)
                | TokenKind::Keyword(KeywordKind::Default)
                | TokenKind::BraceClose
                | TokenKind::Eof
        ) {
            consequent.push(self.parse_statement()?);
        }

        let end = consequent
            .last()
            .map_or(colon_end, |s| s.span().end_usize());

        Ok(SwitchCase {
            test,
            consequent: consequent.into_bump_slice(),
            span: Span::new(start as u32, end as u32),
        })
    }

    /// Parse try statement: `try { block } catch (param) { handler } finally { finalizer }`
    pub(super) fn parse_try_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'try' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Try)
        ));
        self.advance()?;

        // Parse try block
        let block = self.parse_block_statement()?;

        // Parse optional catch clause
        let handler = if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Catch)) {
            Some(self.parse_catch_clause()?)
        } else {
            None
        };

        // Parse optional finally clause
        let finalizer = if matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Finally)
        ) {
            self.advance()?;
            Some(self.parse_block_statement()?)
        } else {
            None
        };

        // Must have at least catch or finally
        if handler.is_none() && finalizer.is_none() {
            return Err(self.error_msg("Missing catch or finally after try"));
        }

        let end = finalizer.as_ref().map_or_else(
            || handler.as_ref().map_or(block.span.end, |h| h.span.end),
            |f| f.span.end,
        );

        Ok(Statement::TryStatement(TryStatement {
            block,
            handler,
            finalizer,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse catch clause: `catch (param) { body }` or `catch { body }`
    fn parse_catch_clause(&mut self) -> Result<CatchClause<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'catch' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Catch)
        ));
        self.advance()?;

        // Parse optional parameter: (param) or (param: type) or ({destructuring}) or ({destructuring}: Type)
        let param = if self.eat(TokenKind::ParenOpen) {
            let param = match self.current_kind() {
                TokenKind::Identifier => {
                    // Simple identifier: catch (e) or catch (e: Error)
                    let (id_start, id_end) = self.current_pos();
                    let symbol = self.intern_identifier();
                    self.advance()?;

                    // Check for type annotation: param: type
                    let (extra, param_end) = if self.check(&TokenKind::Colon) {
                        let ta = self.parse_type_annotation()?;
                        let end = ta.span.end as usize;
                        (Some(self.typed_extra(ta)), end)
                    } else {
                        (None, id_end)
                    };

                    Expression::Identifier(Identifier {
                        name: symbol,
                        optional: false,
                        extra,
                        span: Span::new(id_start as u32, param_end as u32),
                    })
                }
                TokenKind::BraceOpen => {
                    // Object destructuring: catch ({message}) or catch ({message}: ErrorType)
                    let expr = self.parse_object_expression()?;
                    let mut pattern = self.to_assignable(expr)?;

                    // Check for type annotation
                    if self.check(&TokenKind::Colon) {
                        let ta = self.parse_type_annotation()?;
                        let end = ta.span.end;
                        if let Expression::ObjectPattern(ref mut op) = pattern {
                            op.type_annotation = Some(ta);
                            op.span.end = end;
                        }
                    }
                    pattern
                }
                TokenKind::BracketOpen => {
                    // Array destructuring: catch ([x, y]) or catch ([x, y]: ErrorType)
                    let expr = self.parse_array_expression()?;
                    let mut pattern = self.to_assignable(expr)?;

                    // Check for type annotation
                    if self.check(&TokenKind::Colon) {
                        let ta = self.parse_type_annotation()?;
                        let end = ta.span.end;
                        if let Expression::ArrayPattern(ref mut ap) = pattern {
                            ap.type_annotation = Some(ta);
                            ap.span.end = end;
                        }
                    }
                    pattern
                }
                _ => {
                    return Err(self.error_expected("catch parameter"));
                }
            };

            self.expect(&TokenKind::ParenClose)?;
            Some(param)
        } else {
            None
        };

        // Parse body
        let body = self.parse_block_statement()?;
        let end = body.span.end;

        Ok(CatchClause {
            param,
            body,
            span: Span::new(start as u32, end),
        })
    }

    /// Parse throw statement: `throw expr;`
    pub(super) fn parse_throw_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'throw' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Throw)
        ));
        self.advance()?;

        // throw must have an argument (no line terminator allowed between throw and expr)
        // ASI: `throw\nexpr` is a syntax error, not `throw; expr;`
        if self.can_insert_semicolon() {
            return Err(self.error_msg("Illegal newline after throw"));
        }

        let argument = self.parse_expression()?;
        let end = self.semicolon_end()?;

        Ok(Statement::ThrowStatement(ThrowStatement {
            argument,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse break statement: `break;` or `break label;`
    pub(super) fn parse_break_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'break' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Break)
        ));
        self.advance()?;

        // Check for optional label (no line terminator allowed)
        // If ASI can apply, treat as no label
        let label = if !self.can_insert_semicolon()
            && matches!(self.current_kind(), TokenKind::Identifier)
        {
            let (label_start, label_end) = self.current_pos();
            let symbol = self.intern_identifier();
            self.advance()?;
            Some(Identifier::simple(
                symbol,
                Span::new(label_start as u32, label_end as u32),
            ))
        } else {
            None
        };

        let end = self.semicolon_end()?;

        Ok(Statement::BreakStatement(BreakStatement {
            label,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse continue statement: `continue;` or `continue label;`
    pub(super) fn parse_continue_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'continue' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Continue)
        ));
        self.advance()?;

        // Check for optional label (no line terminator allowed)
        // If ASI can apply, treat as no label
        let label = if !self.can_insert_semicolon()
            && matches!(self.current_kind(), TokenKind::Identifier)
        {
            let (label_start, label_end) = self.current_pos();
            let symbol = self.intern_identifier();
            self.advance()?;
            Some(Identifier::simple(
                symbol,
                Span::new(label_start as u32, label_end as u32),
            ))
        } else {
            None
        };

        let end = self.semicolon_end()?;

        Ok(Statement::ContinueStatement(ContinueStatement {
            label,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse debugger statement: `debugger;`
    pub(super) fn parse_debugger_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Debugger)
        ));
        self.advance()?;

        let end = self.semicolon_end()?;

        Ok(Statement::DebuggerStatement(DebuggerStatement {
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse labeled statement: `label: statement`
    pub(super) fn parse_labeled_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, label_end) = self.current_pos();

        // Parse label identifier
        debug_assert!(matches!(self.current_kind(), TokenKind::Identifier));
        let symbol = self.intern_identifier();
        self.advance()?;

        let label = Identifier::simple(symbol, Span::new(start as u32, label_end as u32));

        // Consume ':'
        self.expect(&TokenKind::Colon)?;

        // Parse the labeled statement
        let body = self.arena.alloc(self.parse_statement()?);
        let end = body.span().end;

        Ok(Statement::LabeledStatement(LabeledStatement {
            label,
            body,
            span: Span::new(start as u32, end),
        }))
    }
}
