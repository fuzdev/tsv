// Variable declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
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

        // Parse declarators (comma-separated list)
        let mut declarations = vec![self.parse_variable_declarator()?];
        while self.eat(TokenKind::Comma) {
            declarations.push(self.parse_variable_declarator()?);
        }

        // Span ends at last declarator (ASI may not have explicit semicolon)
        // SAFETY: declarations was initialized with at least one element above
        #[allow(clippy::unwrap_used)]
        let decl_end = declarations.last().unwrap().span.end;
        self.semicolon()?;

        Ok(Statement::VariableDeclaration(VariableDeclaration {
            kind,
            declarations,
            span: Span::new(start as u32, decl_end),
        }))
    }

    pub(super) fn parse_variable_declarator(&mut self) -> Result<VariableDeclarator, ParseError> {
        let id_start = self.current_pos().0;

        // Parse binding pattern: identifier, array pattern [a, b], or object pattern {a, b}
        let id = match self.current_kind() {
            TokenKind::Identifier => {
                // Simple identifier binding
                let (start, end) = self.current_pos();
                let symbol = self.intern(self.current_value());
                self.advance()?;

                // Check for type annotation on identifier
                let type_annotation = if self.check(&TokenKind::Colon) {
                    Some(self.parse_type_annotation()?)
                } else {
                    None
                };

                let id_end = type_annotation
                    .as_ref()
                    .map_or(end, |ta| ta.span.end as usize);

                Expression::Identifier(Identifier {
                    name: symbol,
                    optional: false,
                    type_annotation,
                    span: Span::new(start as u32, id_end as u32),
                })
            }
            TokenKind::BracketOpen => {
                // Array destructuring pattern: [a, b] = arr
                let expr = self.parse_array_expression()?;
                self.to_assignable(expr)?
            }
            TokenKind::BraceOpen => {
                // Object destructuring pattern: {a, b} = obj
                let expr = self.parse_object_expression()?;
                self.to_assignable(expr)?
            }
            _ => {
                return Err(ParseError::InvalidSyntax {
                    message: format!(
                        "Expected identifier or destructuring pattern, found {}",
                        self.current_kind()
                    ),
                    position: self.current_pos().0,
                    context: None,
                });
            }
        };

        let id_end = id.span().end as usize;

        // Check for initializer
        // Use assignment_expression because comma separates declarators
        let init = if self.eat(TokenKind::Equals) {
            Some(self.parse_assignment_expression()?)
        } else {
            None
        };

        let end = init.as_ref().map_or(id_end, |e| e.span().end as usize);

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

        // Parse declarators (comma-separated list)
        let mut declarations = vec![self.parse_variable_declarator()?];
        while self.eat(TokenKind::Comma) {
            declarations.push(self.parse_variable_declarator()?);
        }

        // SAFETY: declarations was initialized with at least one element above
        #[allow(clippy::unwrap_used)]
        let decl_end = declarations.last().unwrap().span.end;

        Ok(VariableDeclaration {
            kind,
            declarations,
            span: Span::new(decl_start as u32, decl_end),
        })
    }
}
