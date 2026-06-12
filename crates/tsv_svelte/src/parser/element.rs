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

/// Check if a tag name is a component (last segment starts with uppercase)
/// Examples: "Comp" -> true, "ns.Comp" -> true, "deep.nested.Comp" -> true, "div" -> false
fn is_component(name: &str) -> bool {
    // For dot notation (ns.Comp), check the last segment
    let last_segment = name.rsplit('.').next().unwrap_or(name);
    last_segment.chars().next().is_some_and(char::is_uppercase)
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
            return Err(self.error_expected_found("tag name"));
        }

        let tag_name = self.current_value().to_string();
        let name_span = Span {
            start: self.current_start as u32,
            end: self.current_end as u32,
        };
        self.advance()?;

        // Check if this is a special element
        if let Some(special_tag) = SpecialElementTag::from_tag_name(&tag_name, in_svelte_head) {
            return self.parse_special_element_body(start, name_span, special_tag);
        }

        // Regular element or component
        let tag_symbol = self.intern(&tag_name);
        let kind = if is_component(&tag_name) {
            ElementKind::Component
        } else {
            ElementKind::Html
        };

        self.parse_regular_element_body(
            start,
            tag_name,
            tag_symbol,
            kind,
            name_span,
            in_svelte_head,
        )
    }

    /// Parse a regular element (HTML or component)
    fn parse_regular_element_body(
        &mut self,
        start: usize,
        tag_name: String,
        tag_symbol: string_interner::DefaultSymbol,
        kind: ElementKind,
        name_span: Span,
        in_svelte_head: bool,
    ) -> Result<ParsedElement, ParseError> {
        // Parse attributes
        let attributes = self.parse_attributes()?;

        // Check for self-closing tag: <div/>
        let self_closing = self.check(TokenKind::Slash);
        if self_closing {
            self.advance()?; // consume /
        }

        // Save positions before consuming > (needed for void/self-closing elements
        // and for the printer to find trailing comments between last attr and >)
        let open_tag_gt = self.current_start as u32;
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
                name_span,
                open_tag_end: open_tag_gt,
            }));
        }

        // Nested <style> and <script> elements have raw text content (not parsed as Svelte template)
        // Per Svelte docs: "the <style> tag will be inserted as-is into the DOM"
        if tag_name == "style" || tag_name == "script" {
            let child_nodes = self.parse_raw_text_content(&tag_name, opening_tag_end, start)?;
            let end = self.parse_closing_tag(&tag_name, start)?;
            return Ok(ParsedElement::Element(Element {
                name: tag_symbol,
                kind,
                attributes,
                fragment: Fragment { nodes: child_nodes },
                span: Span {
                    start: start as u32,
                    end,
                },
                name_span,
                open_tag_end: open_tag_gt,
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
            name_span,
            open_tag_end: open_tag_gt,
        }))
    }

    /// Parse a special element (svelte:*, slot, etc.)
    fn parse_special_element_body(
        &mut self,
        start: usize,
        name_span: Span,
        tag: SpecialElementTag,
    ) -> Result<ParsedElement, ParseError> {
        let tag_name = tag.tag_name();
        let in_svelte_head = tag == SpecialElementTag::SvelteHead;

        // Parse attributes, extracting `this` for SvelteElement and SvelteComponent
        let (attributes, tag_expr, component_expr) = self.parse_special_element_attributes(tag)?;

        // Construct the final SpecialElementKind with associated data
        let kind = Self::build_special_element_kind(tag, tag_expr, component_expr);

        // Check for self-closing tag
        let self_closing = self.check(TokenKind::Slash);
        if self_closing {
            self.advance()?;
        }

        let open_tag_gt = self.current_start as u32;
        let opening_tag_end = self.current_end;
        self.expect(TokenKind::RightAngle)?;

        // Self-closing special elements have no children
        if self_closing {
            return Ok(ParsedElement::SpecialElement(SpecialElement {
                kind,
                attributes,
                fragment: Fragment { nodes: Vec::new() },
                span: Span {
                    start: start as u32,
                    end: opening_tag_end as u32,
                },
                name_span,
                open_tag_end: open_tag_gt,
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
            span: Span {
                start: start as u32,
                end,
            },
            name_span,
            open_tag_end: open_tag_gt,
        }))
    }

    /// Build the final SpecialElementKind from the tag and extracted expressions
    fn build_special_element_kind(
        tag: SpecialElementTag,
        tag_expr: Option<tsv_ts::ast::internal::Expression>,
        component_expr: Option<tsv_ts::ast::internal::Expression>,
    ) -> SpecialElementKind {
        match tag {
            SpecialElementTag::SvelteElement => {
                // For svelte:element, we need the `this` attribute
                // If missing, create a placeholder (parser should have validated)
                let tag = tag_expr.unwrap_or(tsv_ts::ast::internal::Expression::Literal(
                    tsv_ts::ast::internal::Literal {
                        value: tsv_ts::ast::internal::LiteralValue::String {
                            content: String::new(),
                            quote: '"',
                        },
                        span: Span { start: 0, end: 0 },
                    },
                ));
                SpecialElementKind::SvelteElement { tag }
            }
            SpecialElementTag::SvelteComponent => {
                // For svelte:component, we need the `this` attribute
                let expression = component_expr.unwrap_or(
                    tsv_ts::ast::internal::Expression::Literal(tsv_ts::ast::internal::Literal {
                        value: tsv_ts::ast::internal::LiteralValue::Null,
                        span: Span { start: 0, end: 0 },
                    }),
                );
                SpecialElementKind::SvelteComponent { expression }
            }
            SpecialElementTag::SvelteHead => SpecialElementKind::SvelteHead,
            SpecialElementTag::SvelteWindow => SpecialElementKind::SvelteWindow,
            SpecialElementTag::SvelteBody => SpecialElementKind::SvelteBody,
            SpecialElementTag::SvelteDocument => SpecialElementKind::SvelteDocument,
            SpecialElementTag::SvelteSelf => SpecialElementKind::SvelteSelf,
            SpecialElementTag::SlotElement => SpecialElementKind::SlotElement,
            SpecialElementTag::SvelteFragment => SpecialElementKind::SvelteFragment,
            SpecialElementTag::SvelteBoundary => SpecialElementKind::SvelteBoundary,
            SpecialElementTag::TitleElement => SpecialElementKind::TitleElement,
        }
    }

    /// Parse attributes for a special element, extracting `this` for svelte:element and svelte:component
    fn parse_special_element_attributes(
        &mut self,
        tag: SpecialElementTag,
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
                        if tag == SpecialElementTag::SvelteElement {
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
                                                content: t.data().into_owned(),
                                                quote: '"',
                                            },
                                            span: t.span,
                                        },
                                    ));
                                    continue;
                                }
                            }
                        } else if tag == SpecialElementTag::SvelteComponent
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
                last_end = comment.span.end_usize();
                child_nodes.push(FragmentNode::Comment(comment));
            } else if self.check(TokenKind::LeftBrace) {
                let expression_tag = self.parse_expression_tag()?;
                last_end = expression_tag.span.end_usize();
                child_nodes.push(FragmentNode::ExpressionTag(expression_tag));
            } else if self.check(TokenKind::LeftAngle) {
                if self.is_next_token(TokenKind::Slash)? {
                    break;
                }
                // Parse child element (may be special or regular)
                let child = self.parse_element_or_special(in_svelte_head)?;
                match child {
                    ParsedElement::Element(elem) => {
                        last_end = elem.span.end_usize();
                        child_nodes.push(FragmentNode::Element(elem));
                    }
                    ParsedElement::SpecialElement(elem) => {
                        last_end = elem.span.end_usize();
                        child_nodes.push(FragmentNode::SpecialElement(elem));
                    }
                }
            } else if self.check(TokenKind::BlockOpen) {
                let block = self.parse_block()?;
                last_end = block.span().end_usize();
                child_nodes.push(block);
            } else if self.check(TokenKind::TagOpen) {
                let tag = self.parse_template_tag()?;
                last_end = tag.span().end_usize();
                child_nodes.push(tag);
            } else if self.check(TokenKind::Eof) {
                return Err(self.error_unclosed_at(&format!("element: <{tag_name}>"), start));
            } else {
                return Err(self.error_expected_found(
                    "element, expression tag, comment, block, or closing tag",
                ));
            }
        }

        Ok(child_nodes)
    }

    /// Parse closing tag and return end position
    fn parse_closing_tag(&mut self, expected_name: &str, _start: usize) -> Result<u32, ParseError> {
        self.expect(TokenKind::LeftAngle)?;
        self.expect(TokenKind::Slash)?;

        if !self.check(TokenKind::Identifier) {
            return Err(self.error_expected_found("tag name"));
        }

        let closing_tag_name = self.current_value();
        if closing_tag_name != expected_name {
            return Err(self.error_msg(&format!(
                "Mismatched tags: expected closing tag for '{expected_name}' but found '{closing_tag_name}'"
            )));
        }
        self.advance()?;

        let end = self.current_end;
        self.expect(TokenKind::RightAngle)?;

        Ok(end as u32)
    }

    /// Parse raw text content for nested <style> and <script> elements.
    /// These elements should not have their content parsed as Svelte template syntax.
    /// Returns a single Text node with the raw content, or empty vec if no content.
    fn parse_raw_text_content(
        &mut self,
        tag_name: &str,
        content_start: usize,
        element_start: usize,
    ) -> Result<Vec<FragmentNode>, ParseError> {
        // Build the closing tag pattern: </style> or </script>
        let closing_pattern = format!("</{tag_name}>");
        let closing_bytes = closing_pattern.as_bytes();
        let source_bytes = self.source.as_bytes();

        // Scan for the closing tag
        let mut content_end = content_start;
        let mut found_close = false;

        for i in content_start..source_bytes.len() {
            if i + closing_bytes.len() <= source_bytes.len()
                && source_bytes[i..].starts_with(closing_bytes)
            {
                content_end = i;
                found_close = true;
                break;
            }
        }

        if !found_close {
            return Err(
                self.error_msg_at(&format!("Unterminated <{tag_name}> element"), element_start)
            );
        }

        // Reposition lexer to the closing tag
        let remaining_source = &self.source[content_end..];
        let mut new_lexer = crate::lexer::Lexer::new(remaining_source);

        let (token_kind, token_start, token_end) = {
            let token = new_lexer.next_token()?;
            (token.kind, token.start, token.end)
        };

        self.lexer = new_lexer;
        self.base_offset = content_end;
        self.current_kind = token_kind;
        self.current_start = content_end + token_start;
        self.current_end = content_end + token_end;
        self.peek_cache = None;

        // Create a Text node (Svelte always emits one, even if empty)
        let raw_content = &self.source[content_start..content_end];
        Ok(vec![FragmentNode::Text(Text {
            raw: raw_content.to_string(),
            decoding: TextDecoding::Raw,
            span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
        })])
    }
}
