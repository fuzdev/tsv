// Variable declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use string_interner::DefaultSymbol;
use tsv_lang::{ParseError, Span};

use super::super::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_variable_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Get the keyword kind
        let kind = match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Const) => VariableDeclarationKind::Const,
            TokenKind::Keyword(KeywordKind::Let) => VariableDeclarationKind::Let,
            TokenKind::Keyword(KeywordKind::Var) => VariableDeclarationKind::Var,
            _ => unreachable!(),
        };
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

        self.semicolon()?;

        Ok(Statement::VariableDeclaration(VariableDeclaration {
            kind,
            declarations,
            declare: false,
            span: Span::new(start as u32, decl_end),
        }))
    }

    pub(super) fn parse_variable_declarator(&mut self) -> Result<VariableDeclarator, ParseError> {
        let id_start = self.current_pos().0;

        // Parse binding pattern: identifier, array pattern [a, b], or object pattern {a, b}
        // Note: Some keywords can be used as identifiers in variable declarations (e.g., `async`)
        let id = match self.current_kind() {
            TokenKind::Identifier => {
                let symbol = self.intern_identifier();
                self.parse_simple_binding(symbol)?
            }
            // Keywords that can be used as variable names (contextual keywords like `async`)
            TokenKind::Keyword(kw) if kw.can_be_identifier() => {
                let symbol = self.intern(kw.as_str());
                self.parse_simple_binding(symbol)?
            }
            TokenKind::BracketOpen => {
                // Array destructuring pattern: [a, b] = arr
                let expr = self.parse_array_expression()?;
                let mut pattern = self.to_assignable(expr)?;

                // Check for type annotation on array pattern: [a, b]: Type
                if let Expression::ArrayPattern(ref mut arr) = pattern
                    && self.check(&TokenKind::Colon)
                {
                    let type_annotation = self.parse_type_annotation()?;
                    arr.span = Span::new(arr.span.start, type_annotation.span.end);
                    arr.type_annotation = Some(type_annotation);
                }
                pattern
            }
            TokenKind::BraceOpen => {
                // Object destructuring pattern: {a, b} = obj
                let expr = self.parse_object_expression()?;
                let mut pattern = self.to_assignable(expr)?;

                // Check for type annotation on object pattern: {a, b}: Type
                if let Expression::ObjectPattern(ref mut obj) = pattern
                    && self.check(&TokenKind::Colon)
                {
                    let type_annotation = self.parse_type_annotation()?;
                    obj.span = Span::new(obj.span.start, type_annotation.span.end);
                    obj.type_annotation = Some(type_annotation);
                }
                pattern
            }
            _ => {
                return Err(self.error_expected_found("identifier or destructuring pattern"));
            }
        };

        let id_end = id.span().end_usize();

        // Check for initializer
        // Use assignment_expression because comma separates declarators
        let init = if self.eat(TokenKind::Equals) {
            Some(self.parse_assignment_expression()?)
        } else {
            None
        };

        let end = init.as_ref().map_or(id_end, |e| e.span().end_usize());

        Ok(VariableDeclarator {
            id,
            init,
            span: Span::new(id_start as u32, end as u32),
        })
    }

    /// Parse variable declaration for for-loop init (without trailing semicolon)
    pub(super) fn parse_for_variable_declaration(
        &mut self,
    ) -> Result<VariableDeclaration, ParseError> {
        let (decl_start, _) = self.current_pos();

        let kind = match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Const) => VariableDeclarationKind::Const,
            TokenKind::Keyword(KeywordKind::Let) => VariableDeclarationKind::Let,
            TokenKind::Keyword(KeywordKind::Var) => VariableDeclarationKind::Var,
            _ => unreachable!(),
        };
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

        // Parse declarators (comma-separated)
        let first = self.parse_variable_declarator()?;
        let mut decl_end = first.span.end;

        let mut declarations = vec![first];
        while self.eat(TokenKind::Comma) {
            let decl = self.parse_variable_declarator()?;
            decl_end = decl.span.end;
            declarations.push(decl);
        }

        self.semicolon()?;

        Ok(Statement::VariableDeclaration(VariableDeclaration {
            kind: VariableDeclarationKind::Using,
            declarations,
            declare: false,
            span: Span::new(start as u32, decl_end),
        }))
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

        // Parse declarators (comma-separated)
        let first = self.parse_variable_declarator()?;
        let mut decl_end = first.span.end;

        let mut declarations = vec![first];
        while self.eat(TokenKind::Comma) {
            let decl = self.parse_variable_declarator()?;
            decl_end = decl.span.end;
            declarations.push(decl);
        }

        self.semicolon()?;

        Ok(Statement::VariableDeclaration(VariableDeclaration {
            kind: VariableDeclarationKind::AwaitUsing,
            declarations,
            declare: false,
            span: Span::new(start as u32, decl_end),
        }))
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

        // Consume 'await' keyword
        debug_assert!(*self.current_kind() == TokenKind::Keyword(KeywordKind::Await));
        self.advance()?;

        // Consume 'using' contextual keyword
        debug_assert!(self.current_value() == "using");
        self.advance()?;

        // Parse single declarator (for-of only allows one)
        let declarator = self.parse_variable_declarator()?;
        let decl_end = declarator.span.end;

        Ok(VariableDeclaration {
            kind: VariableDeclarationKind::AwaitUsing,
            declarations: vec![declarator],
            declare: false,
            span: Span::new(decl_start as u32, decl_end),
        })
    }

    /// Parse an identifier or contextual keyword as a binding pattern (with optional type annotation)
    ///
    /// Used for variable declarators where the binding is a simple identifier.
    /// Handles both regular identifiers and contextual keywords used as identifiers (e.g., `async`).
    fn parse_simple_binding(&mut self, symbol: DefaultSymbol) -> Result<Expression, ParseError> {
        let (start, end) = self.current_pos();
        self.advance()?;

        let type_annotation = if self.check(&TokenKind::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let id_end = type_annotation
            .as_ref()
            .map_or(end, |ta| ta.span.end_usize());

        Ok(Expression::Identifier(Identifier {
            name: symbol,
            optional: false,
            type_annotation,
            span: Span::new(start as u32, id_end as u32),
        }))
    }
}
