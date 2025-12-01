// Element parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

// Void elements never have closing tags
// Reference: node_modules/svelte/src/utils.js:16-41
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "command", "embed", "hr", "img", "input", "keygen", "link",
    "meta", "param", "source", "track", "wbr",
];

/// Check if an element is void (self-closing by spec, never has children)
fn is_void(name: &str) -> bool {
    VOID_ELEMENTS.contains(&name) || name.eq_ignore_ascii_case("!doctype")
}

/// Check if a tag name is a component (first character uppercase)
fn is_component(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

impl<'a> SvelteParser<'a> {
    /// Parse an element: <tag></tag> or <tag/> or <void>
    pub(crate) fn parse_element(&mut self) -> Result<Element, ParseError> {
        let start = self.current_start;

        // Parse opening tag: <tag>
        self.expect(TokenKind::LeftAngle)?;

        if !self.check(TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected tag name, found {}", self.current_kind),
                position: self.current_start,
                context: None,
            });
        }

        let tag_name = self.current_value().to_string();
        let tag_symbol = self.intern(&tag_name);
        let kind = if is_component(&tag_name) {
            ElementKind::Component
        } else {
            ElementKind::Html
        };
        self.advance()?;

        // Parse attributes
        let attributes = self.parse_attributes()?;

        // Check for self-closing tag: <div/>
        let self_closing = self.check(TokenKind::Slash);
        if self_closing {
            self.advance()?; // consume /
        }

        // Save position before consuming > (needed for void/self-closing elements)
        let opening_tag_end = self.current_end;
        self.expect(TokenKind::RightAngle)?;

        // Void and self-closing elements have no children or closing tag
        if is_void(&tag_name) || self_closing {
            return Ok(Element {
                name: tag_symbol,
                kind,
                attributes,
                fragment: Fragment { nodes: Vec::new() },
                span: Span {
                    start: start as u32,
                    end: opening_tag_end as u32, // Use saved position, not current
                },
            });
        }

        // Parse children
        let mut child_nodes = Vec::new();
        let mut last_end = opening_tag_end;

        // Parse children until we hit closing tag
        #[allow(unused_assignments)] // last_end may be set before loop breaks
        loop {
            // Capture text/whitespace gaps between tokens
            self.capture_text_if_gap(last_end, &mut child_nodes)?;
            // Update last_end to prevent double-capture if we break
            last_end = self.current_start;

            if self.check(TokenKind::Comment) {
                // HTML comment: <!-- ... -->
                let comment = self.parse_comment()?;
                last_end = comment.span.end as usize;
                child_nodes.push(FragmentNode::Comment(comment));
            } else if self.check(TokenKind::LeftBrace) {
                // Expression tag: {expr}
                let expression_tag = self.parse_expression_tag()?;
                last_end = expression_tag.span.end as usize;
                child_nodes.push(FragmentNode::ExpressionTag(expression_tag));
            } else if self.check(TokenKind::LeftAngle) {
                // Check if it's a closing tag or child element
                // Peek ahead: </tag> has slash, <child> doesn't
                if self.is_next_token(TokenKind::Slash)? {
                    // It's a closing tag - exit loop
                    break;
                }
                // It's a child element - recursively parse
                let child = self.parse_element()?;
                last_end = child.span.end as usize;
                child_nodes.push(FragmentNode::Element(child));
            } else if self.check(TokenKind::BlockOpen) {
                // Control flow block: {#if}, {#each}, etc.
                let block = self.parse_block()?;
                last_end = block.span().end as usize;
                child_nodes.push(block);
            } else if self.check(TokenKind::Eof) {
                return Err(ParseError::InvalidSyntax {
                    message: format!("Unclosed element: <{tag_name}>"),
                    position: start,
                    context: None,
                });
            } else {
                // Unexpected token
                return Err(ParseError::InvalidSyntax {
                    message: format!(
                        "Expected element, expression tag, comment, block, or closing tag, found {}",
                        self.current_kind
                    ),
                    position: self.current_start,
                    context: None,
                });
            }
        }

        // Parse closing tag: </tag>
        self.expect(TokenKind::LeftAngle)?;
        self.expect(TokenKind::Slash)?;

        if !self.check(TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected tag name, found {}", self.current_kind),
                position: self.current_start,
                context: None,
            });
        }

        let closing_tag_name = self.current_value();
        if closing_tag_name != tag_name {
            return Err(ParseError::InvalidSyntax {
                message: format!(
                    "Mismatched tags: expected closing tag for '{tag_name}' but found '{closing_tag_name}'"
                ),
                position: self.current_start,
                context: None,
            });
        }
        self.advance()?;

        let end = self.current_end;
        self.expect(TokenKind::RightAngle)?;

        Ok(Element {
            name: tag_symbol,
            kind,
            attributes,
            fragment: Fragment { nodes: child_nodes },
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }
}
