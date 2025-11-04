// Attribute parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

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
        let name_end = self.current_end; // Save end position of name token
        self.advance()?;

        // Check for = (attribute with value)
        if self.check(TokenKind::Equals) {
            self.advance()?; // consume =

            // Parse attribute value (string or expression)
            let value = self.parse_attribute_value()?;

            // Find the end position from the last value part
            let value_end = if let Some(last_part) = value.last() {
                match last_part {
                    AttributeValue::Text(text) => {
                        // For string values, the Text span covers content only (without quotes)
                        // The attribute span must include the closing quote, so add 1 to content_end
                        // Example: type="text" → Text span is "text" (positions 13-17), token is "text" (positions 12-18)
                        text.span.end as usize + 1
                    }
                    AttributeValue::ExpressionTag(tag) => {
                        // Expression span already includes the closing } so use as-is
                        tag.span.end as usize
                    }
                }
            } else {
                return Err(ParseError::InvalidSyntax {
                    message: "Attribute value is empty".to_string(),
                    position: self.current_start,
                    context: None,
                });
            };

            Ok(Attribute {
                name,
                value: Some(value),
                span: Span {
                    start: start as u32,
                    end: value_end as u32,
                },
            })
        } else {
            // Boolean attribute (no value) - ends where the name ends
            Ok(Attribute {
                name,
                value: None,
                span: Span {
                    start: start as u32,
                    end: name_end as u32,
                },
            })
        }
    }

    /// Parse attribute value (e.g., `"ts"` or `{expr}`)
    /// Returns a Vec<AttributeValue> to support mixed text/expressions
    pub(crate) fn parse_attribute_value(&mut self) -> Result<Vec<AttributeValue>, ParseError> {
        let mut parts = Vec::new();

        // Check for expression attribute {expr}
        if self.check(TokenKind::LeftBrace) {
            let expr_tag = self.parse_expression_tag()?;
            parts.push(AttributeValue::ExpressionTag(expr_tag));
            return Ok(parts);
        }

        // Otherwise expect string value
        if !self.check(TokenKind::String) {
            return Err(ParseError::InvalidSyntax {
                message: format!(
                    "Expected string or expression value, found {}",
                    self.current_kind
                ),
                position: self.current_start,
                context: None,
            });
        }

        // Extract string content (without quotes)
        let (token_start, token_end) = self.current_pos();

        // Remove quotes: "ts" -> ts
        let content_start = token_start + 1;
        let content_end = token_end - 1;

        // Extract the actual text content from source
        let text_content = self.source[content_start..content_end].to_string();

        // Decode HTML entities in attribute values (is_attribute_value=true)
        let decoded = tsv_html::decode_character_references(&text_content, true);

        let text = Text {
            raw: text_content,
            data: decoded,
            span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
        };

        self.advance()?;

        parts.push(AttributeValue::Text(text));
        Ok(parts)
    }
}
