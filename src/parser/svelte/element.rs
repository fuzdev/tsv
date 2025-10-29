// Element parsing

use crate::ast::internal::*;
use crate::error::ParseError;
use crate::lexer::svelte::TokenKind;
use crate::span::Span;

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse an element: <tag></tag>
    pub(crate) fn parse_element(&mut self) -> Result<Element, ParseError> {
        let start = self.current_start;

        // Parse opening tag: <tag>
        self.expect(TokenKind::LeftAngle)?;

        if !self.check(TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected tag name, found {}", self.current_kind),
                position: self.current_start,
            });
        }

        let tag_name = self.current_value().to_string();
        let tag_symbol = self.intern(&tag_name);
        self.advance()?;

        // For Sprint 5: no attributes support yet
        self.expect(TokenKind::RightAngle)?;

        // Parse children
        let mut child_nodes = Vec::new();

        // Parse children until we hit closing tag
        loop {
            if self.check(TokenKind::LeftBrace) {
                // Parse expression tag
                let expression_tag = self.parse_expression_tag()?;
                child_nodes.push(FragmentNode::ExpressionTag(expression_tag));
            } else if self.check(TokenKind::LeftAngle) {
                // Could be closing tag or child element
                // For now, assume it's closing tag and break
                // (nested elements not supported in Sprint 6)
                break;
            } else {
                // Unexpected token
                return Err(ParseError::InvalidSyntax {
                    message: format!("Expected element, expression tag, or closing tag, found {}", self.current_kind),
                    position: self.current_start,
                });
            }
        }

        // Now parse the closing tag
        if !self.check(TokenKind::LeftAngle) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected closing tag".to_string(),
                position: self.current_start,
            });
        }

        self.advance()?; // consume <
        if self.check(TokenKind::Slash) {
                // It's a closing tag: </tag>
                self.advance()?;

                if !self.check(TokenKind::Identifier) {
                    return Err(ParseError::InvalidSyntax {
                        message: format!("Expected tag name, found {}", self.current_kind),
                        position: self.current_start,
                    });
                }

                let closing_tag_name = self.current_value();
                if closing_tag_name != tag_name {
                    return Err(ParseError::InvalidSyntax {
                        message: format!("Mismatched tags: expected closing tag for '{}' but found '{}'", tag_name, closing_tag_name),
                        position: self.current_start,
                    });
                }
                self.advance()?;

                // Save end position before advancing past '>'
                let (_, end_after_angle) = self.current_pos();
                self.expect(TokenKind::RightAngle)?;

                let end = end_after_angle;

                return Ok(Element {
                    name: tag_symbol,
                    attributes: Vec::new(),
                    fragment: Fragment { nodes: child_nodes },
                    span: Span { start: start as u32, end: end as u32 },
                });
        }

        // Not a closing tag
        Err(ParseError::InvalidSyntax {
            message: "Expected closing tag with '/'".to_string(),
            position: self.current_start,
        })
    }
}
