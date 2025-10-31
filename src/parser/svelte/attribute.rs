// Attribute parsing

use crate::ast::internal::*;
use crate::error::ParseError;
use crate::lexer::svelte::TokenKind;
use crate::span::Span;

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse attribute list (e.g., `lang="ts" class="foo"`)
    /// Consumes tokens until we hit `>` or `/>`
    pub(crate) fn parse_attributes(&mut self) -> Result<Vec<Attribute>, ParseError> {
        let mut attributes = Vec::new();

        // Parse attributes until we hit > or />
        while !self.check(TokenKind::RightAngle) && !self.check(TokenKind::Slash) {
            if self.check(TokenKind::Identifier) {
                attributes.push(self.parse_attribute()?);
            } else {
                return Err(ParseError::InvalidSyntax {
                    message: format!(
                        "Expected attribute name or '>', found {}",
                        self.current_kind
                    ),
                    position: self.current_start,
                    context: None,
                });
            }
        }

        Ok(attributes)
    }

    /// Parse a single attribute (e.g., `lang="ts"`)
    pub(crate) fn parse_attribute(&mut self) -> Result<Attribute, ParseError> {
        let start = self.current_start;

        // Parse attribute name
        if !self.check(TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected attribute name, found {}", self.current_kind),
                position: self.current_start,
                context: None,
            });
        }

        let name_str = self.current_value().to_string();
        let name = self.intern(&name_str);
        self.advance()?;

        // Check for = (attribute with value)
        if self.check(TokenKind::Equals) {
            self.advance()?; // consume =

            // Parse attribute value
            let value = self.parse_attribute_value()?;
            let end = self.current_start; // After consuming the value

            Ok(Attribute {
                name,
                value: Some(value),
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        } else {
            // Boolean attribute (no value)
            let end = self.current_start;
            Ok(Attribute {
                name,
                value: None,
                span: Span {
                    start: start as u32,
                    end: end as u32,
                },
            })
        }
    }

    /// Parse attribute value (e.g., `"ts"`)
    /// Returns a Vec<AttributeValue> to support mixed text/expressions in future
    pub(crate) fn parse_attribute_value(&mut self) -> Result<Vec<AttributeValue>, ParseError> {
        if !self.check(TokenKind::String) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected string value, found {}", self.current_kind),
                position: self.current_start,
                context: None,
            });
        }

        // Extract string content (without quotes)
        let (start, end) = self.current_pos();

        // Remove quotes: "ts" -> ts
        let content_start = start + 1;
        let content_end = end - 1;

        // Extract the actual text content from source
        let text_content = self.source[content_start..content_end].to_string();

        self.advance()?;

        // TODO(performance): Text node allocates twice (raw + data fields are identical).
        // See TODO_PERF.md "P1: Text Node Dual Storage" and ast/internal.rs:225 for details.
        let text = Text {
            raw: text_content.clone(),
            data: text_content,
            span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
        };

        Ok(vec![AttributeValue::Text(text)])
    }
}
