// Control flow statement parsing (if, for, while, switch, try, throw, break, continue, labeled)

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use crate::parser::expression_assignable::AssignableContext;
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
        let test = self.parse_expression_ref()?;
        self.expect(&TokenKind::ParenClose)?;

        // Parse consequent (can be any statement, including block)
        let consequent = arena.alloc(self.parse_nested_statement()?);

        // Check for optional else clause
        let (alternate, end) =
            if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Else)) {
                self.advance()?; // consume 'else'
                let alt = self.parse_nested_statement()?;
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
        // the keyword's own position: where a non-of head reports its rejection
        let await_at = if is_await {
            Some(self.current_pos().0)
        } else {
            None
        };
        if let Some(at) = await_at {
            // `for await` is an `[+Await]` production. At `Script` goal a position a module
            // would take it in — the top level, and the blocks under it — refuses it, and the
            // refusal is a goal gate: only a module holds the loop there, so the format
            // fallback reads the file as one (`parse_with_goal_or_fallback`). A non-async
            // function body takes it at neither goal and stays the deferred early error.
            if !self.in_await && self.in_await_if_module {
                return Err(self.error_goal_gate_at(
                    "'for await' at the top level is only allowed in a module",
                    at,
                ));
            }
            self.advance()?;
        }

        self.expect(&TokenKind::ParenOpen)?;

        // Parse init/left part - could be:
        // 1. Empty (for (;;))
        // 2. Variable declaration (for (let x = 0; ...))
        // 3. Expression (for (x = 0; ...))
        // 4. Variable declaration for-in/of (for (let x of ...))
        // 5. Expression pattern for-in/of (for (x of ...))

        if self.check(&TokenKind::Semicolon) {
            // Empty init: `for (;;)`. The for-await bar is read at each of the three
            // non-of grammar exits, always before the exit's own `;` — see
            // `reject_for_await_without_of`.
            self.reject_for_await_without_of(await_at)?;
            self.advance()?;
            return self.parse_for_standard(start, None);
        }

        // Check if it starts with a variable declaration. `const` and `var` are
        // declaration keywords outright; `let` is one only when a binding follows it
        // (`Parser::at_let_declaration`) — `for (let in o)`, `for (let; ;)`,
        // `for (let = 3; ;)` and `for (let.x in o)` are expression heads whose leftmost
        // token happens to be the `IdentifierReference` `let`.
        let starts_with_let = matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Let));
        let is_var_decl = matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Const | KeywordKind::Var)
        ) || (starts_with_let && self.at_let_declaration());

        // Check for a `using` / `await using` head (Explicit Resource Management),
        // `for (using resource of resources)` / `for await (await using resource of
        // resources)`. The shared dispatch predicates own the `[no LineTerminator
        // here]` restrictions; a head adds one rule of its own — `[lookahead ≠ of]`,
        // since a using ForBinding cannot be named `of`, so `for (using of items)`
        // is a for-of whose LHS is the plain identifier `using`.
        let is_using = self.at_using_declaration() && self.peek_value() != "of";
        let is_await_using = self.at_await_using_declaration();

        // Neither form has a for-in or C-style spelling, so both parse the same way
        // and differ only in which keyword the rejection names.
        if is_using || is_await_using {
            let var_decl = if is_await_using {
                self.parse_for_await_using_declaration()?
            } else {
                self.parse_for_using_declaration()?
            };

            if self.current_value() == "of" {
                self.advance()?;
                return self.parse_for_of(
                    start,
                    self.arena.alloc(ForInOfLeft::VariableDeclaration(var_decl)),
                    is_await,
                );
            }

            return Err(self.error_msg(if is_await_using {
                "'await using' can only be used in for-of loops"
            } else {
                "'using' can only be used in for-of loops"
            }));
        }

        if is_var_decl {
            // Parse variable declaration (without semicolon)
            let var_decl = self.parse_for_variable_declaration()?;

            // Check for 'in' or 'of'
            let is_for_in = matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::In));
            let is_for_of = self.current_value() == "of";

            // A for-in/for-of head binds exactly one declarator: the grammar is
            // `for ( ForDeclaration in/of … )` where `ForDeclaration` is a single
            // `ForBinding`. Multiple declarators (`for (let a, b of …)`) is a syntax
            // error — acorn reports an unexpected token at the `in`/`of`.
            if (is_for_in || is_for_of) && var_decl.declarations.len() != 1 {
                return Err(self.error_expected_found("a single binding in a for-in/of header"));
            }

            if is_for_in {
                self.advance()?;
                return self.parse_for_in(
                    start,
                    await_at,
                    self.arena.alloc(ForInOfLeft::VariableDeclaration(var_decl)),
                );
            }
            if is_for_of {
                self.advance()?;
                return self.parse_for_of(
                    start,
                    self.arena.alloc(ForInOfLeft::VariableDeclaration(var_decl)),
                    is_await,
                );
            }

            // Standard for loop with var decl init.
            return self.parse_c_style_for(
                start,
                await_at,
                Some(self.arena.alloc(ForInit::VariableDeclaration(var_decl))),
            );
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
            let name = self.current_ident_name();
            self.advance()?; // consume 'async'
            let async_ident = Expression::Identifier(Identifier::simple(
                name,
                Span::new(id_start as u32, id_end as u32),
            ));
            self.advance()?; // consume 'of'
            return self.parse_for_of(
                start,
                self.arena.alloc(ForInOfLeft::Pattern(async_ident)),
                is_await,
            );
        }

        // Parse expression (could be init or left-hand side)
        // Use parse_expression_no_in to prevent `in` from being parsed as binary operator
        let expr = self.parse_expression_no_in()?;

        // Check for 'in' or 'of'. A no-declaration for-in/of LHS is refined
        // through `to_assignable` (the cover-grammar conversion): per the spec
        // an `ObjectLiteral`/`ArrayLiteral` LHS must cover an `AssignmentPattern`
        // (enforcing the rest constraints + producing the deep internal pattern,
        // `ArrayExpression` → `ArrayPattern`), and any other LHS must have a
        // valid (non-`invalid`) assignment-target type.
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::In)) {
            self.advance()?;
            let left = self.to_assignable(expr, AssignableContext::ForHead)?;
            return self.parse_for_in(
                start,
                await_at,
                self.arena.alloc(ForInOfLeft::Pattern(left)),
            );
        }
        if self.current_value() == "of" {
            // `ForInOfStatement`'s of-forms carry `[lookahead ∉ { let, async of }]` (and
            // `[lookahead ≠ let]` for the for-await form), a restriction on the head's
            // leftmost TOKEN whatever shape follows it: `for (let of x)`,
            // `for (let.x of y)` and `for (let[0] of a)` are all syntax errors, and only
            // `for ((let) … of x)` says what they mean. The in-form restricts only
            // `let [` (`[lookahead ≠ let []`), so `for (let in o)` and `for (let.x in o)`
            // are legal there while `for (let[0] in o)` is not. acorn spells this the
            // same way, off the token it recorded before parsing the head. Of the three
            // of-heads only `let.x` reaches this check — `let of` reads as a declaration
            // of `of` and dies at the missing `;`, `let[0]` as an invalid binding pattern.
            if starts_with_let {
                return Err(
                    self.error_msg("The left-hand side of a for-of loop may not start with 'let'")
                );
            }
            self.advance()?;
            let left = self.to_assignable(expr, AssignableContext::ForHead)?;
            return self.parse_for_of(
                start,
                self.arena.alloc(ForInOfLeft::Pattern(left)),
                is_await,
            );
        }

        // Standard for loop with expression init.
        self.parse_c_style_for(
            start,
            await_at,
            Some(self.arena.alloc(ForInit::Expression(expr))),
        )
    }

    /// `for await` heads exactly one production: `ForInOfStatement`'s
    /// `for await ( … of AssignmentExpression ) Statement` and its two binding
    /// spellings. There is no for-await-in head and no for-await C-style head, so every
    /// head that fails to reach a for-of rejects — and the head parse has exactly three
    /// such exits, each of which asks here **before** consuming anything of its own: the
    /// empty init (`for (;;)`), the C-style separator ([`Self::parse_c_style_for`], both
    /// spellings of the init), and the for-in head ([`Self::parse_for_in`], whose entry
    /// *is* its exit).
    ///
    /// "Before" is the load-bearing word, and it is a **diagnostic** requirement rather
    /// than a grammatical one: a malformed head (`for await (a b)`) dies on the separator
    /// first, and that error would report at the stray token instead of on the `await`
    /// keyword acorn puts it on. Pinned by `tests/for_await_rejection.rs`.
    ///
    /// The rejection is a **content** obligation as much as a grammar one: `await` is
    /// printed off the `ForOfStatement`'s own flag, and a `ForInStatement` /
    /// `ForStatement` carries no such field, so a head accepted here would format to
    /// `for (x in o)` with the keyword simply gone. acorn spells the same bar as an
    /// `unexpected(awaitAt)` at each of its exits, and the error lands on the `await`
    /// keyword here too.
    fn reject_for_await_without_of(&self, await_at: Option<usize>) -> Result<(), ParseError> {
        if let Some(at) = await_at {
            return Err(self.error_msg_at("'for await' can only be used in for-of loops", at));
        }
        Ok(())
    }

    /// The C-style exit shared by both init spellings: reject a `for await` head, then
    /// take the separator the exit owes and build. Split out so the two callers cannot
    /// disagree about that order — the rejection has to precede the `expect`, per
    /// [`Self::reject_for_await_without_of`].
    fn parse_c_style_for(
        &mut self,
        start: usize,
        await_at: Option<usize>,
        init: Option<&'arena ForInit<'arena>>,
    ) -> Result<Statement<'arena>, ParseError> {
        self.reject_for_await_without_of(await_at)?;
        self.expect(&TokenKind::Semicolon)?;
        self.parse_for_standard(start, init)
    }

    /// Parse standard for loop: `for (init; test; update) body`. Carries no `await_at`:
    /// a `for await` head is barred at each of the three exits that reach a non-for-of
    /// builder, all of them before this is called
    /// ([`Self::reject_for_await_without_of`]).
    fn parse_for_standard(
        &mut self,
        start: usize,
        init: Option<&'arena ForInit<'arena>>,
    ) -> Result<Statement<'arena>, ParseError> {
        // Parse test (optional)
        let test = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression_ref()?)
        };
        self.expect(&TokenKind::Semicolon)?;

        // Parse update (optional)
        let update = if self.check(&TokenKind::ParenClose) {
            None
        } else {
            Some(self.parse_expression_ref()?)
        };
        self.expect(&TokenKind::ParenClose)?;

        // Parse body
        let body = self.arena.alloc(self.parse_nested_statement()?);
        let end = body.span().end;

        Ok(Statement::ForStatement(ForStatement {
            init,
            test,
            update,
            body,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse for-in loop: `for (left in right) body`. `await_at` is the head's `await`
    /// keyword if it had one — rejected here, since this is not a for-of
    /// (`reject_for_await_without_of`).
    fn parse_for_in(
        &mut self,
        start: usize,
        await_at: Option<usize>,
        left: &'arena ForInOfLeft<'arena>,
    ) -> Result<Statement<'arena>, ParseError> {
        self.reject_for_await_without_of(await_at)?;

        let right = self.parse_expression_ref()?;
        self.expect(&TokenKind::ParenClose)?;

        let body = self.arena.alloc(self.parse_nested_statement()?);
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
        left: &'arena ForInOfLeft<'arena>,
        r#await: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        // A for-of variable declaration may not have an initializer: ecma262's
        // `ForBinding` carries no `Initializer`, and (unlike for-in's `var` head)
        // there is no Annex-B extension, so `for (var/let/const x = 1 of [])` is
        // invalid in every mode — acorn and prettier both reject. This covers the
        // `using` / `await using` heads too (all arrive here as a VariableDeclaration
        // left). A default *inside* a binding pattern (`for (const {x = 1} of [])`) is
        // a pattern default, not a declarator initializer, so it stays valid.
        if matches!(left, ForInOfLeft::VariableDeclaration(decl)
            if decl.declarations.iter().any(|d| d.init.is_some()))
        {
            return Err(
                self.error_msg("for-of loop variable declaration may not have an initializer")
            );
        }

        let right = self.parse_expression_ref()?;
        self.expect(&TokenKind::ParenClose)?;

        let body = self.arena.alloc(self.parse_nested_statement()?);
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
        let (test, body, span) = self.parse_paren_head_statement(KeywordKind::While)?;
        Ok(Statement::WhileStatement(WhileStatement {
            test,
            body,
            span,
        }))
    }

    /// Parse with statement: `with (object) body`
    ///
    /// Sloppy-mode Script code only — the caller reads `Parser::strict` and rejects
    /// ahead of this. Its shape is `while`'s exactly, which is also how prettier
    /// prints it (one printer keyed on the node type).
    pub(super) fn parse_with_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (object, body, span) = self.parse_paren_head_statement(KeywordKind::With)?;
        Ok(Statement::WithStatement(WithStatement {
            object,
            body,
            span,
        }))
    }

    /// Parse the `keyword (expression) body` shape shared by `while` and `with` — the
    /// two statements whose head is one parenthesized expression and whose body is a
    /// single statement. Returns the head expression, the body, and the whole span.
    fn parse_paren_head_statement(
        &mut self,
        keyword: KeywordKind,
    ) -> Result<(&'arena Expression<'arena>, &'arena Statement<'arena>, Span), ParseError> {
        let (start, _) = self.current_pos();

        debug_assert_eq!(self.current_kind(), &TokenKind::Keyword(keyword));
        self.advance()?;

        // Parse the head: (expression)
        self.expect(&TokenKind::ParenOpen)?;
        let head = self.parse_expression_ref()?;
        self.expect(&TokenKind::ParenClose)?;

        // Parse body
        let body = self.arena.alloc(self.parse_nested_statement()?);
        let end = body.span().end;

        Ok((head, body, Span::new(start as u32, end)))
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
        let body = self.arena.alloc(self.parse_nested_statement()?);

        // Expect 'while'
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::While)) {
            return Err(self.error_expected_after("'while'", "do statement body"));
        }
        self.advance()?;

        // Parse condition: (test)
        self.expect(&TokenKind::ParenOpen)?;
        let test = self.parse_expression_ref()?;
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
        let discriminant = self.parse_expression_ref()?;
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
            Some(self.parse_expression_ref()?)
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
            Some(&*self.arena.alloc(self.parse_catch_clause()?))
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
            || handler.map_or(block.span.end, |h| h.span.end),
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
            // A catch binding is a `BindingIdentifier` — a plain identifier, a
            // contextual type keyword (`catch (any)`), or `await` at Script
            // `[~Await]` (all covered by `try_binding_name`) — with an optional
            // `: type` annotation; or a destructuring pattern.
            let param = if let Some(name) = self.try_binding_name() {
                let (id_start, id_end) = self.current_pos();
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
                    escaped_name: name.escaped,
                    name_len: name.raw_len,
                    name_plain_ascii: name.plain_ascii,
                    optional: false,
                    extra,
                    span: Span::new(id_start as u32, param_end as u32),
                })
            } else {
                match self.current_kind() {
                    // Destructuring binding with an optional type annotation:
                    // catch ({message}) / catch ([x, y]) / catch ({message}: ErrorType).
                    // A catch binding takes no optional `?` marker.
                    TokenKind::BraceOpen | TokenKind::BracketOpen => {
                        self.parse_destructured_binding(false)?
                    }
                    _ => {
                        return Err(self.error_expected("catch parameter"));
                    }
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

        let argument = self.parse_expression_ref()?;
        let end = self.semicolon_end()?;

        Ok(Statement::ThrowStatement(ThrowStatement {
            argument,
            span: Span::new(start as u32, end),
        }))
    }

    /// Take the optional `LabelIdentifier` of a `break` / `continue`, or `None`
    /// when ASI ends the statement first (`break [no LineTerminator here]
    /// LabelIdentifier`, so a newline means the label-less form).
    ///
    /// The label set is [`Parser::at_reference_name`] — plain identifiers, the contextual
    /// keywords the lexer turns into `Keyword` tokens (`break async`, `continue
    /// string`), `let` (barred only by the deferred strict-mode early error), `await`
    /// exactly where the goal axis makes it an identifier (`break await` at Script
    /// `[~Await]`, reserved at Module), and `yield` only outside a generator. A bare
    /// `TokenKind::Identifier` test saw only the first group, so a label the
    /// *declaration* site accepted could not be referenced.
    ///
    /// Shared by both statements so the two cannot drift. The *declaration* side
    /// asks the same predicate, but at the statement dispatcher rather than inside
    /// [`Parser::parse_labeled_statement`] — see that function's note.
    fn take_optional_label_reference(&mut self) -> Result<Option<Identifier<'arena>>, ParseError> {
        if self.can_insert_semicolon() || !self.at_reference_name() {
            return Ok(None);
        }
        let (label_start, label_end) = self.current_pos();
        let name = self.current_ident_name_or_await();
        self.advance()?;
        Ok(Some(Identifier::simple(
            name,
            Span::new(label_start as u32, label_end as u32),
        )))
    }

    pub(super) fn parse_break_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'break' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Break)
        ));
        self.advance()?;

        let label = self.take_optional_label_reference()?;

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

        let label = self.take_optional_label_reference()?;

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

        // Parse the label identifier. The `LabelIdentifier` set is
        // `Parser::at_reference_name` — plain identifiers plus every keyword-lexed word
        // the lexer produces that may still be a name here (`async:`, `string:`,
        // `let:`, `await:` at Script `[~Await]`, `yield:` only outside a generator).
        //
        // ⚠️ The gate lives at the *caller* — `parse_statement` dispatches here only
        // when `at_reference_name()` holds — and `try_ident_or_contextual_name` below is
        // a name-BUILDING channel, strictly wider: it applies neither the goal axis
        // to `await` nor the `[~Yield]` production guard to `yield`. So a second
        // caller that skipped the gate would accept `function* g() { yield: ; }`,
        // which must reject. The assert pins that contract at the one caller there
        // is (mirroring `expression_arrow.rs`'s `at_binding_name` assert);
        // `at_reference_name` is a pure `&self` predicate, so it is safe here.
        debug_assert!(self.at_reference_name());
        let name = self
            .try_ident_or_contextual_name()
            .ok_or_else(|| self.error_expected("label"))?;
        self.advance()?;

        let label = Identifier::simple(name, Span::new(start as u32, label_end as u32));

        // Consume ':'
        self.expect(&TokenKind::Colon)?;

        // Parse the labeled statement
        let body_stmt = self.parse_nested_statement()?;

        // `LabelledItem : Statement | FunctionDeclaration`. A lexical declaration
        // (`let`/`const`) and a class declaration are not labelable statements, and
        // neither is a function declaration: the `FunctionDeclaration` arm carries the
        // "It is a Syntax Error if any source text is matched by this production"
        // early error, and the relaxation that lifts it is Annex B §B.3.2 (Labelled
        // Function Declarations) — normative but optional for a non-browser host, and
        // out of tsv's grammar at both goals. So the rejection holds in strict code by
        // the core early error and in sloppy code for want of the carve-out (acorn
        // reports an unexpected token). A `var` statement and ordinary statements are
        // fine; TS declarations (`enum`/`interface`/`type`/`namespace`)
        // acorn-typescript accepts, so they pass through.
        let label_target_invalid = match &body_stmt {
            Statement::ClassDeclaration(_) | Statement::FunctionDeclaration(_) => true,
            Statement::VariableDeclaration(decl) => {
                !matches!(decl.kind, VariableDeclarationKind::Var)
            }
            _ => false,
        };
        if label_target_invalid {
            return Err(self.error_msg_at(
                "A label can only precede a statement, not a declaration",
                body_stmt.span().start as usize,
            ));
        }

        let body = self.arena.alloc(body_stmt);
        let end = body.span().end;

        Ok(Statement::LabeledStatement(LabeledStatement {
            label,
            body,
            span: Span::new(start as u32, end),
        }))
    }
}
