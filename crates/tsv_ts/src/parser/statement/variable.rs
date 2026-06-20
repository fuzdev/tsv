// Variable declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use string_interner::DefaultSymbol;
use tsv_lang::{ParseError, Span};

use super::super::Parser;

impl<'a> Parser<'a> {
    /// Variable kind from the current `const`/`let`/`var` keyword token.
    fn current_variable_kind(&self) -> VariableDeclarationKind {
        match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Const) => VariableDeclarationKind::Const,
            TokenKind::Keyword(KeywordKind::Let) => VariableDeclarationKind::Let,
            TokenKind::Keyword(KeywordKind::Var) => VariableDeclarationKind::Var,
            _ => unreachable!(),
        }
    }

    /// Parse the comma-separated declarator list and trailing semicolon shared
    /// by all variable-declaration statements (`const`/`let`/`var`/`using`/
    /// `await using`), after the keyword(s) have been consumed.
    fn finish_variable_declaration(
        &mut self,
        kind: VariableDeclarationKind,
        start: usize,
    ) -> Result<Statement, ParseError> {
        let mut declarations = vec![self.parse_variable_declarator()?];
        while self.eat(TokenKind::Comma) {
            declarations.push(self.parse_variable_declarator()?);
        }

        let end = self.semicolon_end()?;

        Ok(Statement::VariableDeclaration(VariableDeclaration {
            kind,
            declarations,
            declare: false,
            span: Span::new(start as u32, end),
        }))
    }

    pub(super) fn parse_variable_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();
        let kind = self.current_variable_kind();
        self.advance()?;
        self.finish_variable_declaration(kind, start)
    }

    pub(super) fn parse_variable_declarator(&mut self) -> Result<VariableDeclarator, ParseError> {
        let id_start = self.current_pos().0;

        // Parse binding pattern: identifier, array pattern [a, b], or object pattern {a, b}
        // Note: Some keywords can be used as identifiers in variable declarations (e.g., `async`)
        // For simple identifiers, also handles definite assignment assertion (`!`)
        let (id, definite) = if let Some(symbol) = self.try_intern_binding_name() {
            self.parse_simple_binding(symbol)?
        } else if matches!(
            self.current_kind(),
            TokenKind::BracketOpen | TokenKind::BraceOpen
        ) {
            // Destructuring patterns don't support definite assignment
            (self.parse_destructured_binding()?, false)
        } else {
            return Err(self.error_expected_found("identifier or destructuring pattern"));
        };

        let id_end = id.span().end_usize();

        // Check for initializer
        // Use assignment_expression because comma separates declarators
        let init = if self.eat(TokenKind::Equals) {
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
    ) -> Result<VariableDeclaration, ParseError> {
        let (decl_start, _) = self.current_pos();

        let kind = self.current_variable_kind();
        self.advance()?;

        // Parse first declarator
        let first = self.parse_variable_declarator()?;
        let mut decl_end = first.span.end;

        // Parse additional declarators (comma-separated)
        let mut declarations = vec![first];
        while self.eat(TokenKind::Comma) {
            let decl = self.parse_variable_declarator()?;
            decl_end = decl.span.end;
            declarations.push(decl);
        }

        Ok(VariableDeclaration {
            kind,
            declarations,
            declare: false,
            span: Span::new(decl_start as u32, decl_end),
        })
    }

    /// Parse `using` declaration (ES2024 Explicit Resource Management)
    /// `using resource = getResource();`
    pub(super) fn parse_using_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'using' contextual keyword
        debug_assert!(self.current_value() == "using");
        self.advance()?;

        self.finish_variable_declaration(VariableDeclarationKind::Using, start)
    }

    /// Parse `await using` declaration (ES2024 Explicit Resource Management)
    /// `await using resource = getAsyncResource();`
    pub(super) fn parse_await_using_declaration(&mut self) -> Result<Statement, ParseError> {
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
    ) -> Result<VariableDeclaration, ParseError> {
        let (decl_start, _) = self.current_pos();

        // Consume 'using' contextual keyword
        debug_assert!(self.current_value() == "using");
        self.advance()?;

        // Parse single declarator (for-of only allows one)
        let declarator = self.parse_variable_declarator()?;
        let decl_end = declarator.span.end;

        Ok(VariableDeclaration {
            kind: VariableDeclarationKind::Using,
            declarations: vec![declarator],
            declare: false,
            span: Span::new(decl_start as u32, decl_end),
        })
    }

    /// Parse `await using` declaration for for-await-of loop init (without trailing semicolon)
    /// `for await (await using resource of resources) { ... }`
    pub(super) fn parse_for_await_using_declaration(
        &mut self,
    ) -> Result<VariableDeclaration, ParseError> {
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
    fn parse_simple_binding(
        &mut self,
        symbol: DefaultSymbol,
    ) -> Result<(Expression, bool), ParseError> {
        let (start, end) = self.current_pos();
        self.advance()?;

        // Check for definite assignment assertion: `let x!: Type`
        let definite = self.eat(TokenKind::Bang);

        let type_annotation = self.parse_optional_type_annotation()?;

        let id_end = type_annotation
            .as_ref()
            .map_or(end, |ta| ta.span.end_usize());

        Ok((
            Expression::Identifier(Identifier {
                name: symbol,
                optional: false,
                type_annotation,
                decorators: None,
                span: Span::new(start as u32, id_end as u32),
            }),
            definite,
        ))
    }
}
