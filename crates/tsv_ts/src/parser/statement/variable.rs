// Variable declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

/// Where a variable declarator sits — the two grammar parameters a variable
/// statement and a `for` head disagree on follow from it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DeclaratorSite {
    /// A variable statement: the definite assignment `!` is part of the
    /// production (tsc's `allowExclamation`), and the initializer is `[+In]`.
    Statement,
    /// A `for` head's declaration — the C-style init and the `in`/`of` left
    /// alike, in every keyword spelling: no definite `!`, and the initializer is
    /// `[~In]`.
    ///
    /// tsc reads the marker under three conjuncts (`parseVariableDeclaration`:
    /// `allowExclamation && name.kind === Identifier &&
    /// !scanner.hasPrecedingLineBreak()`), and
    /// `parseVariableDeclarationList(/*inForStatementInitializer*/ true)` selects
    /// the `allowExclamation: false` spelling for the whole head — a grammar
    /// parameter barring a production, so the rejection is the parser's rather
    /// than a deferred early error. acorn-typescript has no such parameter and
    /// accepts, but it is the AST-*shape* oracle, not the validity one. Every
    /// keyword spelling reaches this site through one caller,
    /// `Parser::parse_for_head_declaration`, so the spellings cannot drift apart.
    ///
    /// The `[~In]`: ecma262 parses `ForStatement`'s `LexicalDeclaration[~In]` /
    /// `VariableDeclarationList[~In]`, so a bare `in` ends the initializer
    /// (`for (let a = b in c; ;)` is a syntax error, and `for (var a = 0 in b)`
    /// reaches the for-in branch to be refused there). The restriction reaches
    /// only the initializer's own `AssignmentExpression`: a default inside the
    /// binding pattern is an `Initializer[+In]`, and every bracketing position
    /// under the initializer restores `[+In]` on its own.
    ForHead,
}

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Variable kind from the current `const`/`let`/`var` keyword token.
    pub(super) fn current_variable_kind(&self) -> VariableDeclarationKind {
        match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Const) => VariableDeclarationKind::Const,
            TokenKind::Keyword(KeywordKind::Let) => VariableDeclarationKind::Let,
            TokenKind::Keyword(KeywordKind::Var) => VariableDeclarationKind::Var,
            // Callers only invoke this with the current token on const/let/var.
            #[expect(clippy::unreachable)] // precondition: current token is const/let/var
            _ => unreachable!("current_variable_kind requires a const/let/var keyword token"),
        }
    }

    /// Parse a `const`/`let`/`var` declaration statement.
    pub(super) fn parse_variable_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        self.parse_declaration_statement(self.current_variable_kind())
    }

    /// Parse a declaration statement of `kind` — `const`/`let`/`var`, `using`, or
    /// `await using` (Explicit Resource Management: `using resource =
    /// getResource();`) — from its keyword(s) through the trailing semicolon.
    pub(super) fn parse_declaration_statement(
        &mut self,
        kind: VariableDeclarationKind,
    ) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        self.eat_declaration_keyword(kind)?;
        let (declarations, _) = self.parse_declarator_list(DeclaratorSite::Statement)?;
        let end = self.semicolon_end()?;

        Ok(Statement::from_variable_declaration(VariableDeclaration {
            kind,
            declarations,
            declare: false,
            span: Span::new(start as u32, end),
        }))
    }

    /// Consume `kind`'s keyword token(s) — `await using` is two
    /// ([`VariableDeclarationKind::words`]); the caller has already recognized them.
    fn eat_declaration_keyword(&mut self, kind: VariableDeclarationKind) -> Result<(), ParseError> {
        for word in kind.words() {
            debug_assert!(self.current_value() == *word);
            self.advance()?;
        }
        Ok(())
    }

    /// Parse a comma-separated declarator list, returning it with its end — the
    /// last declarator's, since the list is never empty.
    fn parse_declarator_list(
        &mut self,
        site: DeclaratorSite,
    ) -> Result<(&'arena [VariableDeclarator<'arena>], u32), ParseError> {
        let first = self.parse_declarator(site)?;
        let mut end = first.span.end;
        let mut declarations = self.bvec();
        declarations.push(first);
        while self.eat(TokenKind::Comma) {
            let declarator = self.parse_declarator(site)?;
            end = declarator.span.end;
            declarations.push(declarator);
        }
        Ok((declarations.into_bump_slice(), end))
    }

    /// Parse one variable declarator; `site` settles the two grammar parameters
    /// that differ between a statement and a `for` head.
    fn parse_declarator(
        &mut self,
        site: DeclaratorSite,
    ) -> Result<VariableDeclarator<'arena>, ParseError> {
        let id_start = self.current_pos().0;

        // Parse binding pattern: identifier, array pattern [a, b], or object pattern {a, b}
        // Note: Some keywords can be used as identifiers in variable declarations (e.g., `async`)
        // For simple identifiers, also handles definite assignment assertion (`!`)
        let (id, definite) = if let Some(name) = self.try_binding_name() {
            self.parse_simple_binding(name, site)?
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
        // Both slots are by-reference (see `VariableDeclarator`): the nodes live in
        // the arena and the declarator holds pointers, so the element the
        // declaration's vector moves is 32 B rather than 160.
        let id: &'arena Expression<'arena> = self.alloc(id);

        // Check for initializer
        // Use assignment_expression because comma separates declarators
        let init: Option<&'arena Expression<'arena>> = if !self.eat(TokenKind::Equals) {
            None
        } else if site == DeclaratorSite::ForHead {
            Some(self.with_no_in(Self::parse_assignment_expression_ref)?)
        } else {
            Some(self.parse_assignment_expression_ref()?)
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

    /// Parse a `for` head's declaration (no trailing semicolon) — the C-style
    /// init or the for-in/of left, for every keyword: `var` / `let` / `const`,
    /// `using` (`for (using a = x, b = y; ; )`, `for (using r of rs)`), and
    /// `await using`, whose keyword is two tokens. One grammar serves them all —
    /// a declarator list — and the for-in/of dispatch holds it to one binding.
    pub(super) fn parse_for_head_declaration(
        &mut self,
        kind: VariableDeclarationKind,
    ) -> Result<VariableDeclaration<'arena>, ParseError> {
        let (decl_start, _) = self.current_pos();
        self.eat_declaration_keyword(kind)?;
        let (declarations, decl_end) = self.parse_declarator_list(DeclaratorSite::ForHead)?;

        Ok(VariableDeclaration {
            kind,
            declarations,
            declare: false,
            span: Span::new(decl_start as u32, decl_end),
        })
    }

    /// Parse an identifier or contextual keyword as a binding pattern (with optional type annotation)
    ///
    /// Used for variable declarators where the binding is a simple identifier.
    /// Handles both regular identifiers and contextual keywords used as identifiers (e.g., `async`).
    ///
    /// Returns `(expression, definite)` where `definite` is true if `!` was present.
    ///
    /// `site` is tsc's `allowExclamation`: a `for` head bars the marker by position
    /// (see [`DeclaratorSite::ForHead`]).
    fn parse_simple_binding(
        &mut self,
        name: IdentName<'arena>,
        site: DeclaratorSite,
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
        if definite && site == DeclaratorSite::ForHead {
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
            Expression::from_identifier(Identifier {
                escaped_name: name.escaped,
                name_len: name.raw_len,
                name_plain_ascii: name.plain_ascii,
                optional: false,
                extra,
                span: Span::new(start as u32, id_end as u32),
            }),
            definite,
        ))
    }
}
