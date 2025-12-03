// Class declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_class_declaration(&mut self) -> Result<Statement, ParseError> {
        let class = self.parse_class_declaration_inner(true)?;
        Ok(Statement::ClassDeclaration(class))
    }

    /// Inner function that returns the ClassDeclaration directly
    /// Used by both parse_class_declaration and export default
    ///
    /// `name_required`: If true, class name is required. If false, name is optional
    /// (for `export default class {}`)
    pub(super) fn parse_class_declaration_inner(
        &mut self,
        name_required: bool,
    ) -> Result<ClassDeclaration, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'class' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Class)
        ));
        self.advance()?;

        // Parse class name (required for declarations, optional for export default)
        let id = if matches!(self.current_kind(), TokenKind::Identifier) {
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern(self.current_value());
            self.advance()?;

            Some(Identifier {
                name: symbol,
                optional: false,
                type_annotation: None,
                span: Span::new(id_start as u32, id_end as u32),
            })
        } else if name_required {
            return Err(ParseError::InvalidSyntax {
                message: "Expected class name after 'class'".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        } else {
            None
        };

        // Parse optional `extends` clause
        let super_class = if matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Extends)
        ) {
            self.advance()?; // consume 'extends'

            // Parse superclass expression (typically an identifier, but could be member expression)
            let expr = self.parse_expression()?;
            Some(Box::new(expr))
        } else {
            None
        };

        // Parse class body
        let body = self.parse_class_body()?;
        let end = body.span.end;

        Ok(ClassDeclaration {
            id,
            super_class,
            body,
            span: Span::new(start as u32, end),
        })
    }

    fn parse_class_body(&mut self) -> Result<ClassBody, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BraceOpen)?;

        let mut body = Vec::new();

        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            let member = self.parse_class_member()?;
            body.push(member);
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(ClassBody {
            body,
            span: Span::new(start as u32, end as u32),
        })
    }

    fn parse_class_member(&mut self) -> Result<ClassMember, ParseError> {
        let (start, _) = self.current_pos();

        // Handle 'static' contextual keyword
        let is_static = self.eat_contextual_keyword("static");

        // Handle 'get' and 'set' contextual keywords for getters/setters
        let accessor_kind = if matches!(self.current_kind(), TokenKind::Identifier) {
            let kind = match self.current_value() {
                "get" => Some(MethodKind::Get),
                "set" => Some(MethodKind::Set),
                _ => None,
            };
            // Peek ahead to see if next token is an identifier or bracket (accessor)
            // vs `(` or `=` (property named 'get'/'set')
            if kind.is_some()
                && (self.peek_is_identifier() || matches!(self.peek_kind(), TokenKind::BracketOpen))
            {
                self.advance().ok();
                kind
            } else {
                None
            }
        } else {
            None
        };

        // Parse member name (key)
        let (computed, key, method_name) = if matches!(self.current_kind(), TokenKind::BracketOpen)
        {
            // Computed key: [expr]
            self.advance()?;
            let expr = self.parse_expression()?;
            self.expect(&TokenKind::BracketClose)?;
            (true, expr, None)
        } else if matches!(self.current_kind(), TokenKind::Identifier) {
            // Regular identifier key - capture name before advancing
            let name_str = self.current_value().to_string();
            let (key_start, key_end) = self.current_pos();
            let symbol = self.intern(&name_str);
            self.advance()?;
            (
                false,
                Expression::Identifier(Identifier {
                    name: symbol,
                    optional: false,
                    type_annotation: None,
                    span: Span::new(key_start as u32, key_end as u32),
                }),
                Some(name_str),
            )
        } else {
            return Err(ParseError::InvalidSyntax {
                message: "Expected class member name".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        };

        // Detect if this is a method (has `(`) or property (has `=` or `;` or end of class)
        if matches!(self.current_kind(), TokenKind::ParenOpen) {
            // Method definition - use accessor_kind if set, otherwise check for constructor
            let kind = accessor_kind.unwrap_or(match method_name.as_deref() {
                Some("constructor") => MethodKind::Constructor,
                _ => MethodKind::Method,
            });

            // Parse parameter list and block body (like a function)
            let params = self.parse_parameter_list()?;

            // Check for return type annotation: (): type
            let return_type = if self.check(&TokenKind::Colon) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            let body_block = self.parse_block_statement()?;
            let end = body_block.span.end;

            // Create FunctionExpression for the method value
            // TODO: Handle async/generator methods in class declarations
            let value = FunctionExpression {
                id: None,
                params,
                return_type,
                body: body_block,
                generator: false,
                r#async: false,
                span: Span::new(start as u32, end),
            };

            Ok(ClassMember::MethodDefinition(MethodDefinition {
                key,
                value,
                kind,
                is_static,
                computed,
                span: Span::new(start as u32, end),
            }))
        } else {
            // Property definition: `name: type = value;` or `name: type;` or `name = value;` or `name;`

            // Check for type annotation: `name: type`
            let type_annotation = if self.check(&TokenKind::Colon) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            // Check for value: `= value`
            let value = if self.eat(TokenKind::Equals) {
                Some(self.parse_assignment_expression()?)
            } else {
                None
            };

            let end = value.as_ref().map_or_else(
                || {
                    type_annotation
                        .as_ref()
                        .map_or_else(|| key.span().end, |ta| ta.span.end)
                },
                |v| v.span().end,
            );

            // Consume optional semicolon (ASI applies)
            self.eat(TokenKind::Semicolon);

            Ok(ClassMember::PropertyDefinition(PropertyDefinition {
                key,
                type_annotation,
                value,
                is_static,
                computed,
                span: Span::new(start as u32, end),
            }))
        }
    }
}
