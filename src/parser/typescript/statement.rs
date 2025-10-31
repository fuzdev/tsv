// Statement parsing

use crate::ast::internal::*;
use crate::error::ParseError;
use crate::lexer::{KeywordKind, TokenKind};
use crate::span::Span;

use super::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        // Check if this is a variable declaration
        match self.current_kind() {
            TokenKind::Keyword(kw) => match kw {
                KeywordKind::Const | KeywordKind::Let | KeywordKind::Var => {
                    self.parse_variable_declaration()
                }
                KeywordKind::Number => {
                    // Type keywords are not valid at statement level
                    Err(ParseError::InvalidSyntax {
                        message: format!("Unexpected keyword '{}'", kw),
                        position: self.current_pos().0,
                        context: None,
                    })
                }
            },
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                let expr_span = expr.span();
                let semi_end = self.current_pos().1;
                self.expect(TokenKind::Semicolon)?;
                Ok(Statement::ExpressionStatement(ExpressionStatement {
                    expression: expr,
                    span: Span::new(expr_span.start, semi_end as u32),
                }))
            }
        }
    }

    fn parse_variable_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Get the keyword kind
        let kind = match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Const) => VariableDeclarationKind::Const,
            TokenKind::Keyword(KeywordKind::Let) => VariableDeclarationKind::Let,
            TokenKind::Keyword(KeywordKind::Var) => VariableDeclarationKind::Var,
            _ => unreachable!(),
        };
        self.advance()?;

        // Parse declarator (for now, only single declarator)
        // TODO: Support multiple declarators (const a = 1, b = 2;)
        let declarator = self.parse_variable_declarator()?;
        let declarations = vec![declarator];

        let semi_end = self.current_pos().1;
        self.expect(TokenKind::Semicolon)?;

        Ok(Statement::VariableDeclaration(VariableDeclaration {
            kind,
            declarations,
            span: Span::new(start as u32, semi_end as u32),
        }))
    }

    fn parse_variable_declarator(&mut self) -> Result<VariableDeclarator, ParseError> {
        // Parse identifier
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected identifier in variable declaration".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern(self.current_value());
        self.advance()?;

        // Check for type annotation
        let type_annotation = if self.check(TokenKind::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        // Calculate the end position of identifier (including type annotation if present)
        let id_span_end = type_annotation
            .as_ref()
            .map(|ta| ta.span.end as usize)
            .unwrap_or(id_end);

        let id = Identifier {
            name: symbol,
            type_annotation,
            span: Span::new(id_start as u32, id_span_end as u32),
        };

        // Check for initializer
        let init = if self.check(TokenKind::Equals) {
            self.advance()?;
            Some(self.parse_expression()?)
        } else {
            None
        };

        let end = init
            .as_ref()
            .map(|e| e.span().end as usize)
            .unwrap_or(id_span_end);

        Ok(VariableDeclarator {
            id,
            init,
            span: Span::new(id_start as u32, end as u32),
        })
    }

    fn parse_type_annotation(&mut self) -> Result<TSTypeAnnotation, ParseError> {
        let start = self.current_pos().0;
        self.expect(TokenKind::Colon)?;

        let type_node = self.parse_type()?;
        let end = type_node.span().end;

        Ok(TSTypeAnnotation {
            type_annotation: Box::new(type_node),
            span: Span::new(start as u32, end),
        })
    }

    fn parse_type(&mut self) -> Result<TSType, ParseError> {
        match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Number) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(TSType::TSNumberKeyword(TSNumberKeyword {
                    span: Span::new(start as u32, end as u32),
                }))
            }
            _ => Err(ParseError::InvalidSyntax {
                message: format!("Expected type, found {}", self.current_kind()),
                position: self.current_pos().0,
                context: None,
            }),
        }
    }
}
