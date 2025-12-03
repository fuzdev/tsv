// TypeScript type parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::super::Parser;

impl<'a> Parser<'a> {
    pub(in crate::parser) fn parse_type_annotation(
        &mut self,
    ) -> Result<TSTypeAnnotation, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::Colon)?;

        let type_node = self.parse_type()?;
        let end = type_node.span().end;

        Ok(TSTypeAnnotation {
            type_annotation: Box::new(type_node),
            span: Span::new(start as u32, end),
        })
    }

    pub(super) fn parse_type(&mut self) -> Result<TSType, ParseError> {
        let (start, end) = self.current_pos();
        let span = Span::new(start as u32, end as u32);

        // Parse base type
        let mut result = if let TokenKind::Keyword(kw) = self.current_kind()
            && let Some(ts_kind) = TSKeywordKind::from_lexer_keyword(kw)
        {
            self.advance()?;
            TSType::Keyword(TSKeywordType::new(ts_kind, span))
        } else if matches!(
            self.current_kind(),
            TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead
        ) {
            // Check for template literal type: `hello` or `hello ${T} world`
            let template = self.parse_template_literal_type()?;
            TSType::Literal(TSLiteralType::TemplateLiteral(template))
        } else {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected type, found {}", self.current_kind()),
                position: self.current_pos().0,
                context: None,
            });
        };

        // Check for array type suffix: []
        while self.check(&TokenKind::BracketOpen) {
            self.advance()?; // consume '['
            let (_, arr_end) = self.current_pos();
            self.expect(&TokenKind::BracketClose)?;

            result = TSType::Array(TSArrayType {
                element_type: Box::new(result),
                span: Span::new(start as u32, arr_end as u32),
            });
        }

        Ok(result)
    }

    /// Parse a template literal in type context: `hello ${string} world`
    ///
    /// Parallel structure to `parse_template_literal()` in expression.rs but parses
    /// types inside ${} instead of expressions. Kept separate for clarity despite
    /// duplication - the two contexts (expression vs type) rarely change together.
    fn parse_template_literal_type(&mut self) -> Result<TemplateLiteralType, ParseError> {
        let (start, _) = self.current_pos();
        let mut quasis = Vec::new();
        let mut types = Vec::new();

        match self.current_kind() {
            TokenKind::NoSubstitutionTemplate => {
                // Simple template with no interpolation: `hello world`
                let (elem_start, elem_end) = self.current_pos();
                let raw = self.current_value().to_string();

                // Extract content between backticks
                let content = if raw.len() >= 2 {
                    raw[1..raw.len() - 1].to_string()
                } else {
                    String::new()
                };

                // Decode escapes for cooked value
                let cooked = if let Some(decoded) = self.current_decoded() {
                    Some(decoded.to_string())
                } else {
                    Some(content.clone())
                };

                self.advance()?;

                quasis.push(TemplateElement {
                    raw: content,
                    cooked,
                    tail: true,
                    span: Span::new(elem_start as u32, elem_end as u32),
                });

                Ok(TemplateLiteralType {
                    quasis,
                    types,
                    span: Span::new(start as u32, elem_end as u32),
                })
            }
            TokenKind::TemplateHead => {
                // Template with interpolation: `hello ${string}...`
                let (elem_start, elem_end) = self.current_pos();
                let raw = self.current_value().to_string();

                // Extract content: remove leading ` and trailing ${
                let content = if raw.len() >= 3 {
                    raw[1..raw.len() - 2].to_string()
                } else {
                    String::new()
                };

                let cooked = if let Some(decoded) = self.current_decoded() {
                    Some(decoded.to_string())
                } else {
                    Some(content.clone())
                };

                self.advance()?;

                quasis.push(TemplateElement {
                    raw: content,
                    cooked,
                    tail: false,
                    span: Span::new(elem_start as u32, elem_end as u32),
                });

                // Parse types and remaining template parts
                loop {
                    // Parse the interpolated type (not expression!)
                    let ts_type = self.parse_type()?;
                    types.push(ts_type);

                    // Expect closing } of the interpolation
                    let (brace_start, _) = self.current_pos();
                    if !self.check(&TokenKind::BraceClose) {
                        return Err(ParseError::InvalidSyntax {
                            message: "Expected '}' after type in template literal".to_string(),
                            position: self.current_pos().0,
                            context: None,
                        });
                    }

                    // Use lexer to continue template from }
                    let token = self
                        .lexer
                        .continue_template_from_brace(self.current_raw_end())?;
                    self.update_current(token);

                    match self.current_kind().clone() {
                        TokenKind::TemplateTail => {
                            // Final part: }content`
                            let (_tail_start, tail_end) = self.current_pos();
                            let tail_raw = self.current_value().to_string();

                            // Extract content: remove leading } and trailing `
                            let tail_content = if tail_raw.len() >= 2 {
                                tail_raw[1..tail_raw.len() - 1].to_string()
                            } else {
                                String::new()
                            };

                            let tail_cooked = if let Some(decoded) = self.current_decoded() {
                                Some(decoded.to_string())
                            } else {
                                Some(tail_content.clone())
                            };

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw: tail_content,
                                cooked: tail_cooked,
                                tail: true,
                                span: Span::new(brace_start as u32, tail_end as u32),
                            });

                            return Ok(TemplateLiteralType {
                                quasis,
                                types,
                                span: Span::new(start as u32, tail_end as u32),
                            });
                        }
                        TokenKind::TemplateMiddle => {
                            // Middle part: }content${
                            let (_mid_start, mid_end) = self.current_pos();
                            let mid_raw = self.current_value().to_string();

                            // Extract content: remove leading } and trailing ${
                            let mid_content = if mid_raw.len() >= 3 {
                                mid_raw[1..mid_raw.len() - 2].to_string()
                            } else {
                                String::new()
                            };

                            let mid_cooked = if let Some(decoded) = self.current_decoded() {
                                Some(decoded.to_string())
                            } else {
                                Some(mid_content.clone())
                            };

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw: mid_content,
                                cooked: mid_cooked,
                                tail: false,
                                span: Span::new(brace_start as u32, mid_end as u32),
                            });
                            // Continue loop for next interpolation
                        }
                        _ => {
                            return Err(ParseError::InvalidSyntax {
                                message: "Unexpected token in template literal type".to_string(),
                                position: self.current_pos().0,
                                context: None,
                            });
                        }
                    }
                }
            }
            _ => Err(ParseError::InvalidSyntax {
                message: format!(
                    "Expected template literal type, found {}",
                    self.current_kind()
                ),
                position: self.current_pos().0,
                context: None,
            }),
        }
    }

    pub(super) fn parse_type_alias_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'type' contextual keyword
        debug_assert!(self.current_value() == "type");
        self.advance()?;

        // Parse type name (identifier)
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected type name after 'type'".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern(self.current_value());
        self.advance()?;

        let id = Identifier {
            name: symbol,
            optional: false,
            type_annotation: None,
            span: Span::new(id_start as u32, id_end as u32),
        };

        // Expect '='
        self.expect(&TokenKind::Equals)?;

        // Parse the type
        let type_annotation = self.parse_type()?;
        let type_end = type_annotation.span().end;
        self.semicolon()?;

        Ok(Statement::TSTypeAliasDeclaration(TSTypeAliasDeclaration {
            id,
            type_annotation,
            span: Span::new(start as u32, type_end),
        }))
    }
}
