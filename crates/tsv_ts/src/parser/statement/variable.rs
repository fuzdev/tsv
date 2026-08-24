// Variable declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Variable kind from the current `const`/`let`/`var` keyword token.
    fn current_variable_kind(&self) -> VariableDeclarationKind {
        match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Const) => VariableDeclarationKind::Const,
            TokenKind::Keyword(KeywordKind::Let) => VariableDeclarationKind::Let,
            TokenKind::Keyword(KeywordKind::Var) => VariableDeclarationKind::Var,
            // Callers only invoke this with the current token on const/let/var.
            #[expect(clippy::unreachable)] // precondition: current token is const/let/var
            _ => unreachable!("current_variable_kind requires a const/let/var keyword token"),
        }
    }

    /// Parse the comma-separated declarator list and trailing semicolon shared
    /// by all variable-declaration statements (`const`/`let`/`var`/`using`/
    /// `await using`), after the keyword(s) have been consumed.
    fn finish_variable_declaration(
        &mut self,
        kind: VariableDeclarationKind,
        start: usize,
    ) -> Result<Statement<'arena>, ParseError> {
        let mut declarations = self.bvec();
        declarations.push(self.parse_variable_declarator()?);
        while self.eat(TokenKind::Comma) {
            declarations.push(self.parse_variable_declarator()?);
        }

        let end = self.semicolon_end()?;

        Ok(Statement::VariableDeclaration(VariableDeclaration {
            kind,
            declarations: declarations.into_bump_slice(),
            declare: false,
            span: Span::new(start as u32, end),
        }))
    }

    pub(super) fn parse_variable_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        let kind = self.current_variable_kind();
        self.advance()?;
        self.finish_variable_declaration(kind, start)
    }

    /// Parse one declarator of a variable **statement**, where the definite
    /// assignment `!` is part of the declarator production.
    fn parse_variable_declarator(&mut self) -> Result<VariableDeclarator<'arena>, ParseError> {
        self.parse_declarator(true)
    }

    /// Parse one declarator of a `for` **head** — the C-style init and the
    /// `in`/`of` left alike, in every keyword spelling — where the definite
    /// assignment `!` is *not* part of the production.
    ///
    /// tsc reads the marker under three conjuncts (`parseVariableDeclaration`:
    /// `allowExclamation && name.kind === Identifier &&
    /// !scanner.hasPrecedingLineBreak()`), and
    /// `parseVariableDeclarationList(/*inForStatementInitializer*/ true)` selects
    /// the `allowExclamation: false` spelling for the whole head — a grammar
    /// parameter barring a production, so the rejection is the parser's rather
    /// than a deferred early error. acorn-typescript has no such parameter and
    /// accepts, but it is the AST-*shape* oracle, not the validity one.
    ///
    /// The rule is stated here rather than at each caller so the keyword
    /// spellings cannot drift apart: `let`/`const`/`var` reach it through
    /// [`Self::parse_for_variable_declaration`], `using` and `await using`
    /// through [`Self::parse_for_using_declaration`].
    fn parse_for_header_declarator(&mut self) -> Result<VariableDeclarator<'arena>, ParseError> {
        self.parse_declarator(false)
    }

    /// Shared declarator body. `allow_definite` is tsc's `allowExclamation`.
    fn parse_declarator(
        &mut self,
        allow_definite: bool,
    ) -> Result<VariableDeclarator<'arena>, ParseError> {
        let id_start = self.current_pos().0;

        // Parse binding pattern: identifier, array pattern [a, b], or object pattern {a, b}
        // Note: Some keywords can be used as identifiers in variable declarations (e.g., `async`)
        // For simple identifiers, also handles definite assignment assertion (`!`)
        let (id, definite) = if let Some(name) = self.try_binding_name() {
            self.parse_simple_binding(name, allow_definite)?
        } else if matches!(
            self.current_kind(),
            TokenKind::BracketOpen | TokenKind::BraceOpen
        ) {
            // Destructuring patterns don't support definite assignment. No
            // optional `?`: `const []? = x` is invalid (rejected by both parsers).
            (self.parse_destructured_binding(false)?, false)
        } else {
            return Err(self.error_expected_found("identifier or destructuring pattern"));
        };

        let id_end = id.span().end_usize();

        // Check for initializer
        // Use assignment_expression because comma separates declarators
        let init: Option<Expression<'arena>> = if self.eat(TokenKind::Equals) {
            Some(self.parse_assignment_expression()?)
        } else {
            None
        };

        // Use the later of expression span end and prev_token_end() to include any
        // stripped parens (e.g., JSDoc type cast: `const a = /** @type {T} */ (expr)` —
        // the closing `)` is consumed by the parser but not part of the inner expression's
        // span). Using max() handles both the normal case (same value) and error recovery
        // (expression span may extend further). Matches acorn's VariableDeclarator span.
        // Without an initializer, prev_token_end() likewise extends past the id span when a
        // definite assignment `!` was consumed without a type annotation (`let a!;`).
        let end = init.as_ref().map_or_else(
            || id_end.max(self.prev_token_end()),
            |e| e.span().end_usize().max(self.prev_token_end()),
        );

        Ok(VariableDeclarator {
            id,
            init,
            definite,
            span: Span::new(id_start as u32, end as u32),
        })
    }

    /// Parse variable declaration for for-loop init (without trailing semicolon)
    pub(super) fn parse_for_variable_declaration(
        &mut self,
    ) -> Result<VariableDeclaration<'arena>, ParseError> {
        let (decl_start, _) = self.current_pos();

        let kind = self.current_variable_kind();
        self.advance()?;

        // Parse first declarator
        let first = self.parse_for_header_declarator()?;
        let mut decl_end = first.span.end;

        // Parse additional declarators (comma-separated)
        let mut declarations = self.bvec();
        declarations.push(first);
        while self.eat(TokenKind::Comma) {
            let decl = self.parse_for_header_declarator()?;
            decl_end = decl.span.end;
            declarations.push(decl);
        }

        Ok(VariableDeclaration {
            kind,
            declarations: declarations.into_bump_slice(),
            declare: false,
            span: Span::new(decl_start as u32, decl_end),
        })
    }

    /// Parse `using` declaration (Explicit Resource Management)
    /// `using resource = getResource();`
    pub(super) fn parse_using_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'using' contextual keyword
        debug_assert!(self.current_value() == "using");
        self.advance()?;

        self.finish_variable_declaration(VariableDeclarationKind::Using, start)
    }

    /// Parse `await using` declaration (Explicit Resource Management)
    /// `await using resource = getAsyncResource();`
    pub(super) fn parse_await_using_declaration(
        &mut self,
    ) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'await' keyword
        debug_assert!(*self.current_kind() == TokenKind::Keyword(KeywordKind::Await));
        self.advance()?;

        // Consume 'using' contextual keyword
        debug_assert!(self.current_value() == "using");
        self.advance()?;

        self.finish_variable_declaration(VariableDeclarationKind::AwaitUsing, start)
    }

    /// Parse `using` declaration for for-of loop init (without trailing semicolon)
    /// `for (using resource of resources) { ... }`
    pub(super) fn parse_for_using_declaration(
        &mut self,
    ) -> Result<VariableDeclaration<'arena>, ParseError> {
        let (decl_start, _) = self.current_pos();

        // Consume 'using' contextual keyword
        debug_assert!(self.current_value() == "using");
        self.advance()?;

        // Parse single declarator (for-of only allows one)
        let declarator = self.parse_for_header_declarator()?;
        let decl_end = declarator.span.end;

        let mut declarations = self.bvec();
        declarations.push(declarator);
        Ok(VariableDeclaration {
            kind: VariableDeclarationKind::Using,
            declarations: declarations.into_bump_slice(),
            declare: false,
            span: Span::new(decl_start as u32, decl_end),
        })
    }

    /// Parse `await using` declaration for for-await-of loop init (without trailing semicolon)
    /// `for await (await using resource of resources) { ... }`
    pub(super) fn parse_for_await_using_declaration(
        &mut self,
    ) -> Result<VariableDeclaration<'arena>, ParseError> {
        let (decl_start, _) = self.current_pos();

        // Consume 'await' keyword, then delegate to the `using` form
        debug_assert!(*self.current_kind() == TokenKind::Keyword(KeywordKind::Await));
        self.advance()?;

        let mut decl = self.parse_for_using_declaration()?;
        decl.kind = VariableDeclarationKind::AwaitUsing;
        decl.span = Span::new(decl_start as u32, decl.span.end);
        Ok(decl)
    }

    /// Parse an identifier or contextual keyword as a binding pattern (with optional type annotation)
    ///
    /// Used for variable declarators where the binding is a simple identifier.
    /// Handles both regular identifiers and contextual keywords used as identifiers (e.g., `async`).
    ///
    /// Returns `(expression, definite)` where `definite` is true if `!` was present.
    ///
    /// `allow_definite` is tsc's `allowExclamation` — false in a `for` head, where
    /// the marker is barred by position (see [`Self::parse_for_header_declarator`]).
    fn parse_simple_binding(
        &mut self,
        name: IdentName<'arena>,
        allow_definite: bool,
    ) -> Result<(Expression<'arena>, bool), ParseError> {
        let (start, end) = self.current_pos();
        self.advance()?;

        // Check for definite assignment assertion: `let x!: Type`. The `!` must not
        // be preceded by a line terminator (TS `BindingIdentifier [no LineTerminator
        // here] !`): a newline before it makes it not a definite-assignment assertion,
        // leaving `!` a stray token (acorn-typescript's `hasPrecedingLineBreak` guard).
        // Same rule as the arrow `=>` / conditional `extends` / predicate `is`.
        let marker_start = self.current_pos().0;
        let definite = !self.had_line_terminator && self.eat(TokenKind::Bang);

        // The position conjunct of the same guard. Rejecting rather than dropping the
        // token: the printer prints a binding through the plain expression path, which
        // cannot see `definite`, so accepting here deleted authored source and emitted
        // a program that re-parsed differently.
        if definite && !allow_definite {
            return Err(self.error_msg_at(
                "a definite assignment assertion is not permitted in a for header",
                marker_start,
            ));
        }

        let type_annotation = self.parse_optional_type_annotation()?;

        let id_end = type_annotation
            .as_ref()
            .map_or(end, |ta| ta.span.end_usize());

        let extra = type_annotation.map(|ta| self.typed_extra(ta));

        Ok((
            Expression::Identifier(Identifier {
                escaped_name: name.escaped,
                name_len: name.raw_len,
                optional: false,
                extra,
                span: Span::new(start as u32, id_end as u32),
            }),
            definite,
        ))
    }
}
