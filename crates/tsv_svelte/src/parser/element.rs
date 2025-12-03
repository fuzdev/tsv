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

/// Result type for parsing elements - either a regular element or a special element
pub(crate) enum ParsedElement {
    Element(Element),
    SpecialElement(SpecialElement),
}

/// Result of parsing special element attributes: (attributes, tag_expr for svelte:element, component_expr for svelte:component)
type SpecialElementAttrs = (
    Vec<AttributeNode>,
    Option<tsv_ts::ast::internal::Expression>,
    Option<tsv_ts::ast::internal::Expression>,
);

impl<'a> SvelteParser<'a> {
    /// Parse an element or special element: <tag></tag> or <tag/> or <void>
    ///
    /// Detects special elements (svelte:*, slot) and parses them appropriately.
    /// Returns a ParsedElement enum to distinguish between regular and special elements.
    pub(crate) fn parse_element_or_special(
        &mut self,
        in_svelte_head: bool,
    ) -> Result<ParsedElement, ParseError> {
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
        self.advance()?;

        // Check if this is a special element
        if let Some(special_kind) = SpecialElementKind::from_tag_name(&tag_name, in_svelte_head) {
            return self.parse_special_element_body(start, special_kind);
        }

        // Regular element or component
        let tag_symbol = self.intern(&tag_name);
        let kind = if is_component(&tag_name) {
            ElementKind::Component
        } else {
            ElementKind::Html
        };

        self.parse_regular_element_body(start, tag_name, tag_symbol, kind, in_svelte_head)
    }

    /// Parse a regular element (HTML or component)
    fn parse_regular_element_body(
        &mut self,
        start: usize,
        tag_name: String,
        tag_symbol: string_interner::DefaultSymbol,
        kind: ElementKind,
        in_svelte_head: bool,
    ) -> Result<ParsedElement, ParseError> {
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
            return Ok(ParsedElement::Element(Element {
                name: tag_symbol,
                kind,
                attributes,
                fragment: Fragment { nodes: Vec::new() },
                span: Span {
                    start: start as u32,
                    end: opening_tag_end as u32,
                },
            }));
        }

        // Parse children
        let child_nodes = self.parse_children(&tag_name, opening_tag_end, start, in_svelte_head)?;

        // Parse closing tag: </tag>
        let end = self.parse_closing_tag(&tag_name, start)?;

        Ok(ParsedElement::Element(Element {
            name: tag_symbol,
            kind,
            attributes,
            fragment: Fragment { nodes: child_nodes },
            span: Span {
                start: start as u32,
                end,
            },
        }))
    }

    /// Parse a special element (svelte:*, slot, etc.)
    fn parse_special_element_body(
        &mut self,
        start: usize,
        kind: SpecialElementKind,
    ) -> Result<ParsedElement, ParseError> {
        let tag_name = kind.tag_name();
        let in_svelte_head = kind == SpecialElementKind::SvelteHead;

        // Parse attributes
        let (attributes, tag_expr, component_expr) = self.parse_special_element_attributes(kind)?;

        // Check for self-closing tag
        let self_closing = self.check(TokenKind::Slash);
        if self_closing {
            self.advance()?;
        }

        let opening_tag_end = self.current_end;
        self.expect(TokenKind::RightAngle)?;

        // Self-closing special elements have no children
        if self_closing {
            return Ok(ParsedElement::SpecialElement(SpecialElement {
                kind,
                attributes,
                fragment: Fragment { nodes: Vec::new() },
                tag: tag_expr,
                expression: component_expr,
                span: Span {
                    start: start as u32,
                    end: opening_tag_end as u32,
                },
            }));
        }

        // Parse children
        let child_nodes = self.parse_children(tag_name, opening_tag_end, start, in_svelte_head)?;

        // Parse closing tag
        let end = self.parse_closing_tag(tag_name, start)?;

        Ok(ParsedElement::SpecialElement(SpecialElement {
            kind,
            attributes,
            fragment: Fragment { nodes: child_nodes },
            tag: tag_expr,
            expression: component_expr,
            span: Span {
                start: start as u32,
                end,
            },
        }))
    }

    /// Parse attributes for a special element, extracting `this` for svelte:element and svelte:component
    fn parse_special_element_attributes(
        &mut self,
        kind: SpecialElementKind,
    ) -> Result<SpecialElementAttrs, ParseError> {
        let mut attributes = Vec::new();
        let mut tag_expr: Option<tsv_ts::ast::internal::Expression> = None;
        let mut component_expr: Option<tsv_ts::ast::internal::Expression> = None;

        // Parse all attributes
        let all_attrs = self.parse_attributes()?;

        for attr in all_attrs {
            match &attr {
                AttributeNode::Attribute(a) => {
                    let attr_name = self
                        .interner
                        .borrow()
                        .resolve(a.name)
                        .map(str::to_owned)
                        .unwrap_or_default();
                    // Check for `this` attribute on svelte:element and svelte:component
                    if attr_name == "this" {
                        if kind == SpecialElementKind::SvelteElement {
                            // Extract expression from the attribute value
                            if let Some(ref values) = a.value {
                                if let Some(AttributeValue::ExpressionTag(et)) = values.first() {
                                    tag_expr = Some(et.expression.clone());
                                    continue; // Don't add to attributes
                                } else if let Some(AttributeValue::Text(t)) = values.first() {
                                    // String value: create a literal expression
                                    tag_expr = Some(tsv_ts::ast::internal::Expression::Literal(
                                        tsv_ts::ast::internal::Literal {
                                            value: tsv_ts::ast::internal::LiteralValue::String {
                                                content: t.data.clone(),
                                                quote: '"',
                                            },
                                            span: t.span,
                                        },
                                    ));
                                    continue;
                                }
                            }
                        } else if kind == SpecialElementKind::SvelteComponent
                            && let Some(ref values) = a.value
                            && let Some(AttributeValue::ExpressionTag(et)) = values.first()
                        {
                            // Extract expression from the attribute value
                            component_expr = Some(et.expression.clone());
                            continue; // Don't add to attributes
                        }
                    }
                    attributes.push(attr);
                }
                _ => attributes.push(attr),
            }
        }

        Ok((attributes, tag_expr, component_expr))
    }

    /// Parse children until closing tag
    fn parse_children(
        &mut self,
        tag_name: &str,
        opening_tag_end: usize,
        start: usize,
        in_svelte_head: bool,
    ) -> Result<Vec<FragmentNode>, ParseError> {
        let mut child_nodes = Vec::new();
        let mut last_end = opening_tag_end;

        #[allow(unused_assignments)]
        loop {
            // Capture text/whitespace gaps between tokens
            self.capture_text_if_gap(last_end, &mut child_nodes)?;
            last_end = self.current_start;

            if self.check(TokenKind::Comment) {
                let comment = self.parse_comment()?;
                last_end = comment.span.end as usize;
                child_nodes.push(FragmentNode::Comment(comment));
            } else if self.check(TokenKind::LeftBrace) {
                let expression_tag = self.parse_expression_tag()?;
                last_end = expression_tag.span.end as usize;
                child_nodes.push(FragmentNode::ExpressionTag(expression_tag));
            } else if self.check(TokenKind::LeftAngle) {
                if self.is_next_token(TokenKind::Slash)? {
                    break;
                }
                // Parse child element (may be special or regular)
                let child = self.parse_element_or_special(in_svelte_head)?;
                match child {
                    ParsedElement::Element(elem) => {
                        last_end = elem.span.end as usize;
                        child_nodes.push(FragmentNode::Element(elem));
                    }
                    ParsedElement::SpecialElement(elem) => {
                        last_end = elem.span.end as usize;
                        child_nodes.push(FragmentNode::SpecialElement(elem));
                    }
                }
            } else if self.check(TokenKind::BlockOpen) {
                let block = self.parse_block()?;
                last_end = block.span().end as usize;
                child_nodes.push(block);
            } else if self.check(TokenKind::TagOpen) {
                let tag = self.parse_template_tag()?;
                last_end = tag.span().end as usize;
                child_nodes.push(tag);
            } else if self.check(TokenKind::Eof) {
                return Err(ParseError::InvalidSyntax {
                    message: format!("Unclosed element: <{tag_name}>"),
                    position: start,
                    context: None,
                });
            } else {
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

        Ok(child_nodes)
    }

    /// Parse closing tag and return end position
    fn parse_closing_tag(&mut self, expected_name: &str, _start: usize) -> Result<u32, ParseError> {
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
        if closing_tag_name != expected_name {
            return Err(ParseError::InvalidSyntax {
                message: format!(
                    "Mismatched tags: expected closing tag for '{expected_name}' but found '{closing_tag_name}'"
                ),
                position: self.current_start,
                context: None,
            });
        }
        self.advance()?;

        let end = self.current_end;
        self.expect(TokenKind::RightAngle)?;

        Ok(end as u32)
    }

    /// Parse an element: <tag></tag> or <tag/> or <void>
    ///
    /// Legacy wrapper that only returns regular elements.
    /// For special elements, they are parsed but returned as a Component element.
    /// This maintains backward compatibility with existing callers.
    ///
    /// TODO: Migrate callers to use parse_element_or_special directly and remove this.
    #[allow(dead_code)]
    pub(crate) fn parse_element(&mut self) -> Result<Element, ParseError> {
        match self.parse_element_or_special(false)? {
            ParsedElement::Element(elem) => Ok(elem),
            ParsedElement::SpecialElement(special) => {
                // Convert special element to a regular element for backward compatibility
                // This loses the special element semantics but preserves the parse
                Ok(Element {
                    name: self.intern(special.kind.tag_name()),
                    kind: ElementKind::Component, // Treat as component
                    attributes: special.attributes,
                    fragment: special.fragment,
                    span: special.span,
                })
            }
        }
    }
}
