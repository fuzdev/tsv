// Element parsing

use bumpalo::collections::Vec as BumpVec;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;
use super::{find_exact_tag_close, rcdata_close_at};

/// Check if a tag name is a component.
///
/// A dotted tag (member access, e.g. `ns.Comp`, `Object.component`, `object.property`) is always
/// a component; otherwise it's a component iff the first char is uppercase. Mirrors Svelte's
/// `regex_valid_component_name` (`1-parse/state/element.js`): uppercase-first with optional dots,
/// or any `ID_Start`-first name with one or more dotted segments.
///
/// Examples: "Comp" -> true, "ns.Comp" -> true, "Object.component" -> true, "object.property" ->
/// true, "div" -> false
fn is_component(name: &str) -> bool {
    name.contains('.') || name.chars().next().is_some_and(char::is_uppercase)
}

/// Result type for parsing elements - either a regular element or a special element.
pub(crate) enum ParsedElement<'arena> {
    Element(Element<'arena>),
    SpecialElement(SpecialElement<'arena>),
}

/// Result of parsing special element attributes: (attributes, tag_expr for svelte:element, component_expr for svelte:component)
type SpecialElementAttrs<'arena> = (
    BumpVec<'arena, AttributeNode<'arena>>,
    Option<tsv_ts::ast::internal::Expression<'arena>>,
    Option<tsv_ts::ast::internal::Expression<'arena>>,
);

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Whether an attribute list carries a `shadowrootmode` attribute (by name). On a
    /// RegularElement this marks a declarative-shadow-root template, so descendant `<slot>`s are
    /// ordinary elements rather than `SlotElement`s — mirrors Svelte's
    /// `parent_is_shadowroot_template` (`1-parse/state/element.js`).
    fn attrs_have_shadowrootmode(&self, attributes: &[AttributeNode<'arena>]) -> bool {
        let interner = self.interner.borrow();
        attributes.iter().any(|attr| {
            matches!(attr, AttributeNode::Attribute(a) if interner.resolve(a.name) == Some("shadowrootmode"))
        })
    }

    /// Parse an element or special element: <tag></tag> or <tag/> or <void>
    ///
    /// Detects special elements (svelte:*, slot) and parses them appropriately.
    /// Returns a ParsedElement enum to distinguish between regular and special elements.
    pub(crate) fn parse_element_or_special(&mut self) -> Result<ParsedElement<'arena>, ParseError> {
        let start = self.current_start;

        // Parse opening tag: <tag>
        self.expect(TokenKind::LeftAngle)?;

        if !self.check(TokenKind::Identifier) {
            return Err(self.error_expected_found("tag name"));
        }

        // `&'a str` borrows the source, so it survives the `&mut self` calls below.
        let tag_name = self.current_value();
        let name_span = Span {
            start: self.current_start as u32,
            end: self.current_end as u32,
        };
        self.advance()?;

        // Check if this is a special element. `title`/`slot` classification depends on the
        // ancestor context tracked on the parser (see `SpecialElementTag::from_tag_name`).
        if let Some(special_tag) = SpecialElementTag::from_tag_name(
            tag_name,
            self.in_svelte_head,
            self.in_shadowroot_template,
        ) {
            return self.parse_special_element_body(start, name_span, special_tag);
        }

        // Regular element or component
        let tag_symbol = self.intern(tag_name);
        let kind = if is_component(tag_name) {
            ElementKind::Component
        } else {
            ElementKind::Html
        };

        self.parse_regular_element_body(start, tag_name, tag_symbol, kind, name_span)
    }

    /// Parse a regular element (HTML or component)
    fn parse_regular_element_body(
        &mut self,
        start: usize,
        tag_name: &'a str,
        tag_symbol: string_interner::DefaultSymbol,
        kind: ElementKind,
        name_span: Span,
    ) -> Result<ParsedElement<'arena>, ParseError> {
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

        // Resolve this element's children and end offset. The four content regimes differ
        // only in how they produce `(nodes, end)`; the element is assembled once below.
        let (nodes, end): (&'arena [FragmentNode<'arena>], u32) =
            if tsv_html::is_void_element(tag_name) || self_closing {
                // Void and self-closing elements have no children or closing tag
                // (classification lives in tsv_html, shared with the printer).
                (&[], opening_tag_end as u32)
            } else if tag_name == "style" || tag_name == "script" {
                // Nested <style>/<script> are raw text (not parsed as Svelte template) —
                // per Svelte docs, "the <style> tag will be inserted as-is into the DOM".
                let child_nodes = self.parse_raw_text_content(tag_name, opening_tag_end, start)?;
                let end = self.parse_closing_tag(tag_name)?;
                (child_nodes.into_bump_slice(), end)
            } else if tag_name == "textarea" {
                // <textarea> is RCDATA: raw text with live {expr} interpolation up to a
                // whitespace/attribute-tolerant </textarea…>, where `<` is literal text
                // (never a nested element). Svelte's sole RCDATA element — a sibling of the
                // <script>/<style> raw-text branch above, but interleaving Text +
                // ExpressionTag chunks.
                let (child_nodes, end) = self.parse_rcdata_content(opening_tag_end, start)?;
                (child_nodes.into_bump_slice(), end)
            } else {
                // Parse children. Only HTML elements participate in HTML5 implicit tag
                // closing (Svelte gates auto-close on `parent.type === 'RegularElement'`);
                // components and `svelte:*` keep the strict explicit-close requirement.
                // `parse_children` resolves `end` — past this element's `</tag>` (explicit
                // close) or at the `<` that implicitly closed it.
                let is_html = matches!(kind, ElementKind::Html);
                // Enter this element's ancestor context: a RegularElement/Component resets head
                // context (mirrors Svelte's `parent_is_head`), and a RegularElement carrying
                // `shadowrootmode` turns on shadow-root-template context for its subtree
                // (`parent_is_shadowroot_template`).
                let in_shadow = self.in_shadowroot_template
                    || (is_html && self.attrs_have_shadowrootmode(&attributes));
                let (child_nodes, end) = self.parse_children_in_context(
                    false,
                    in_shadow,
                    tag_name,
                    opening_tag_end,
                    start,
                    is_html,
                )?;
                (child_nodes.into_bump_slice(), end)
            };

        Ok(ParsedElement::Element(Element {
            name: tag_symbol,
            kind,
            attributes: attributes.into_bump_slice(),
            fragment: Fragment { nodes },
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
    ) -> Result<ParsedElement<'arena>, ParseError> {
        let tag_name = tag.tag_name();

        // Parse attributes, extracting `this` for SvelteElement and SvelteComponent
        let (attributes, tag_expr, component_expr) = self.parse_special_element_attributes(tag)?;

        // Construct the final SpecialElementKind with associated data
        let kind = self.build_special_element_kind(tag, tag_expr, component_expr);

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
                attributes: attributes.into_bump_slice(),
                fragment: Fragment { nodes: &[] },
                span: Span {
                    start: start as u32,
                    end: opening_tag_end as u32,
                },
                name_span,
                open_tag_end: open_tag_gt,
            }));
        }

        // Parse children. Special elements (`svelte:*`, `slot`, …) are not HTML
        // RegularElements, so they never auto-close (`is_html = false`) — a
        // mismatched close falls to `parse_closing_tag`'s strict error.
        //
        // Ancestor context: `<svelte:head>` turns head context on; every other special element is
        // transparent (Svelte's `parent_is_head`/`parent_is_shadowroot_template` only stop at a
        // RegularElement/Component), so both flags carry through unchanged otherwise.
        let in_head = tag == SpecialElementTag::SvelteHead || self.in_svelte_head;
        let (child_nodes, end) = self.parse_children_in_context(
            in_head,
            self.in_shadowroot_template,
            tag_name,
            opening_tag_end,
            start,
            false,
        )?;

        Ok(ParsedElement::SpecialElement(SpecialElement {
            kind,
            attributes: attributes.into_bump_slice(),
            fragment: Fragment {
                nodes: child_nodes.into_bump_slice(),
            },
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
        &self,
        tag: SpecialElementTag,
        tag_expr: Option<tsv_ts::ast::internal::Expression<'arena>>,
        component_expr: Option<tsv_ts::ast::internal::Expression<'arena>>,
    ) -> SpecialElementKind<'arena> {
        match tag {
            SpecialElementTag::SvelteElement => {
                // For svelte:element, we need the `this` attribute
                // If missing, create a placeholder (parser should have validated).
                // Use a `Decoded("")` empty-string cooked value so `resolve` reads
                // the arena bytes directly (a `Verbatim` form would slice the span
                // minus quotes and underflow on this zero-length placeholder span).
                let tag = tag_expr.unwrap_or_else(|| {
                    tsv_ts::ast::internal::Expression::Literal(tsv_ts::ast::internal::Literal {
                        value: tsv_ts::ast::internal::LiteralValue::String(
                            tsv_ts::ast::internal::StringCooked::Decoded(self.alloc_str_in("")),
                        ),
                        span: Span { start: 0, end: 0 },
                    })
                });
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
    ) -> Result<SpecialElementAttrs<'arena>, ParseError> {
        let mut attributes = self.bvec();
        let mut tag_expr: Option<tsv_ts::ast::internal::Expression<'arena>> = None;
        let mut component_expr: Option<tsv_ts::ast::internal::Expression<'arena>> = None;

        // Parse all attributes
        let all_attrs = self.parse_attributes()?;

        for attr in all_attrs {
            match &attr {
                AttributeNode::Attribute(a) => {
                    // Check for `this` attribute on svelte:element and svelte:component.
                    // Compare the resolved name by borrow — no per-attribute `String`.
                    if self.interner.borrow().resolve(a.name) == Some("this") {
                        if tag == SpecialElementTag::SvelteElement {
                            // Extract expression from the attribute value
                            if let Some(values) = a.value {
                                if let Some(AttributeValue::ExpressionTag(et)) = values.first() {
                                    tag_expr = Some(et.expression.clone());
                                    continue; // Don't add to attributes
                                } else if let Some(AttributeValue::Text(t)) = values.first() {
                                    // String value: create a literal expression. The
                                    // decoded text is copied once into the arena as a
                                    // `Decoded` cooked value (the source slice carries
                                    // entities / no quotes, so it is not `Verbatim`).
                                    let content = self.alloc_str_in(&t.data(self.source));
                                    tag_expr = Some(tsv_ts::ast::internal::Expression::Literal(
                                        tsv_ts::ast::internal::Literal {
                                            value: tsv_ts::ast::internal::LiteralValue::String(
                                                tsv_ts::ast::internal::StringCooked::Decoded(
                                                    content,
                                                ),
                                            ),
                                            span: t.span,
                                        },
                                    ));
                                    continue;
                                }
                            }
                        } else if tag == SpecialElementTag::SvelteComponent
                            && let Some(values) = a.value
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

    /// Parse an element's children under a given ancestor context (`in_svelte_head` /
    /// `in_shadowroot_template`), restoring the caller's context afterward — so the context is
    /// scoped to this subtree and siblings are unaffected (Svelte's stack push/pop). Delegates to
    /// [`Self::parse_children`] for the actual parse; the save/restore is the only added work. The
    /// restore also runs on the error path, though the parse aborts then anyway.
    fn parse_children_in_context(
        &mut self,
        in_svelte_head: bool,
        in_shadowroot_template: bool,
        tag_name: &str,
        opening_tag_end: usize,
        start: usize,
        is_html: bool,
    ) -> Result<(BumpVec<'arena, FragmentNode<'arena>>, u32), ParseError> {
        let saved_head = self.in_svelte_head;
        let saved_shadow = self.in_shadowroot_template;
        self.in_svelte_head = in_svelte_head;
        self.in_shadowroot_template = in_shadowroot_template;
        let result = self.parse_children(tag_name, opening_tag_end, start, is_html);
        self.in_svelte_head = saved_head;
        self.in_shadowroot_template = saved_shadow;
        result
    }

    /// Parse children until this element's end is resolved, returning the child
    /// nodes and the element's end byte offset. The end is either past this
    /// element's own `</tag>` (consumed here via `parse_closing_tag`) or, under
    /// HTML5 implicit tag closing, the offset of the `<` that triggered the implicit
    /// close (an ancestor's `</other>` or an auto-closing sibling `<next>`, left
    /// unconsumed for the caller's caller to re-read — matching Svelte's
    /// `parent.end = start`). `is_html` enables the auto-close rules: only HTML
    /// `RegularElement`s participate (see the callers). Callers that establish an ancestor
    /// context (head / shadowroot-template) should go through [`Self::parse_children_in_context`].
    fn parse_children(
        &mut self,
        tag_name: &str,
        opening_tag_end: usize,
        start: usize,
        is_html: bool,
    ) -> Result<(BumpVec<'arena, FragmentNode<'arena>>, u32), ParseError> {
        let mut child_nodes = self.bvec();
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
                let tag = self.parse_brace_tag()?;
                last_end = tag.span().end_usize();
                child_nodes.push(tag);
            } else if self.check(TokenKind::LeftAngle) {
                // A `<…>` may end this element rather than nest inside it. Both
                // auto-close paths leave the triggering `<` unconsumed for the
                // caller's caller and end this element at its offset.
                // TODO: a diagnostics layer would record the implicit close at these
                // two points (Svelte's `element_implicitly_closed` warning).
                if self.is_next_token(TokenKind::Slash)? {
                    // A closing tag `</name>`. Our own close → consume it. An HTML
                    // element's mismatched close is an ancestor's → leave it, so it
                    // unwinds to the matching ancestor (or errors at the root if none
                    // matches). A non-HTML parent takes the strict mismatch error.
                    let end = if is_html && !self.is_closing_tag_for(tag_name) {
                        self.current_start as u32
                    } else {
                        self.parse_closing_tag(tag_name)?
                    };
                    return Ok((child_nodes, end));
                }
                // An opening tag `<next>` that the optional-end-tag table says closes
                // this element — leave it for the parent to adopt as a sibling.
                if is_html
                    && let Some(next_name) = self.peek_open_tag_name()?
                    && tsv_html::closing_tag_omitted(tag_name, Some(next_name))
                {
                    return Ok((child_nodes, self.current_start as u32));
                }
                // Parse child element (may be special or regular)
                let child = self.parse_element_or_special()?;
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
    }

    /// Whether the `</…>` closing tag at the current `<` names `tag_name`.
    ///
    /// The parser is positioned at the `<` of a closing tag (current token
    /// `LeftAngle`, next `Slash`); this reads the name straight from source
    /// without consuming tokens, so a non-matching (ancestor's) close can be left
    /// in place for the caller to re-read. The name must be exactly `tag_name`
    /// followed by a tag-name terminator (whitespace, `/`, `>`, or EOF), so `</li>`
    /// matches `li` but `</link>` does not.
    fn is_closing_tag_for(&self, tag_name: &str) -> bool {
        let name_start = self.current_start + 2; // past `</`
        if !self
            .source
            .get(name_start..)
            .is_some_and(|rest| rest.starts_with(tag_name))
        {
            return false;
        }
        match self.source.as_bytes().get(name_start + tag_name.len()) {
            None => true,
            Some(b) => b.is_ascii_whitespace() || *b == b'/' || *b == b'>',
        }
    }

    /// The tag name of the opening tag at the current `<` (peeked, not consumed).
    ///
    /// Returns `None` when the token after `<` is not an identifier (e.g. `<!…`,
    /// `<>`), in which case there is no name to test against the auto-close table.
    /// The returned `&'a str` borrows the immutable source, so it survives the
    /// `&mut self` borrow (same pattern as `current_value`).
    fn peek_open_tag_name(&mut self) -> Result<Option<&'a str>, ParseError> {
        if self.peek.is_none() {
            self.peek = Some(self.lexer.next_token()?);
        }
        Ok(self.peek.as_ref().and_then(|p| {
            (p.kind == TokenKind::Identifier).then(|| {
                &self.source[self.base_offset + p.start as usize..self.base_offset + p.end as usize]
            })
        }))
    }

    /// Consume a `</name>` closing tag (lexer positioned at `<`) and return the byte
    /// offset past `>`. Shared by the generic element path and the raw-text
    /// `<script>` / `<style>` parsers, so all three agree on tag-close tokenization
    /// (whitespace before `>` skipped by the lexer, mismatch → one error).
    pub(super) fn parse_closing_tag(&mut self, expected_name: &str) -> Result<u32, ParseError> {
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
    ) -> Result<BumpVec<'arena, FragmentNode<'arena>>, ParseError> {
        // Nested raw-text uses an EXACT `</tag>` close (no `\s*` before `>`), matching
        // Svelte's generic element parser — unlike a top-level `<script>`/`<style>`, which
        // reads via the whitespace-tolerant `find_raw_text_close`. See that function.
        let content_end =
            find_exact_tag_close(self.source.as_bytes(), content_start, tag_name.as_bytes())
                .ok_or_else(|| {
                    self.error_msg_at(&format!("Unterminated <{tag_name}> element"), element_start)
                })?;

        // Reposition the lexer to the closing tag. We resume at `<`, which lexes to
        // `LeftAngle` in either mode; `inside_tag` is `false` here (after the opening
        // tag's `>`), which `advance_to_position` preserves.
        self.advance_to_position(content_end)?;

        // Create a Text node (Svelte always emits one, even if empty)
        let span = Span {
            start: content_start as u32,
            end: content_end as u32,
        };
        let mut nodes = self.bvec();
        nodes.push(FragmentNode::Text(Text::new(
            span,
            TextDecoding::Raw,
            span,
            self.source,
        )));
        Ok(nodes)
    }

    /// Parse RCDATA content for `<textarea>` — Svelte's sole RCDATA element (verified
    /// against the oracle: `<title>` is *not* RCDATA in Svelte; its children parse as a
    /// normal fragment). RCDATA (HTML §13.2.5.2) is raw text with live `{expr}`
    /// interpolation but no nested elements: `<` is literal text, read up to a
    /// whitespace/attribute-tolerant `</textarea…>`. Ports Svelte's `read_sequence`
    /// (`1-parse/state/element.js`): scan the content bytes, flushing `Text` chunks and,
    /// at each `{`, parsing an `ExpressionTag`.
    ///
    /// Returns the child nodes and the element end (byte offset past the close tag's `>`),
    /// and repositions the lexer there. Not routed through `parse_closing_tag` — the close
    /// tag may carry whitespace/attributes (`</textarea data-x >`) that its exact
    /// tokenization rejects.
    fn parse_rcdata_content(
        &mut self,
        content_start: usize,
        element_start: usize,
    ) -> Result<(BumpVec<'arena, FragmentNode<'arena>>, u32), ParseError> {
        // `&'a [u8]` borrowed from the immutable source, so it survives the `&mut self`
        // expression-tag parse below (its lifetime is the source's, not this borrow's).
        let bytes = self.source.as_bytes();
        let mut nodes = self.bvec();
        let mut chunk_start = content_start;
        let mut i = content_start;

        let close_gt = loop {
            // A `</textarea…>` at `i` ends the RCDATA (checked first, like `read_sequence`'s
            // `done()`); flush the pending text and stop.
            if let Some((close_lt, close_gt)) = rcdata_close_at(bytes, i, b"textarea") {
                self.push_rcdata_text(&mut nodes, chunk_start, close_lt);
                break close_gt;
            }
            match bytes.get(i) {
                // EOF before any close — Svelte's `unexpected_eof` (both parsers reject).
                None => {
                    return Err(self.error_unclosed_at("<textarea> element", element_start));
                }
                // `{expr}` — flush the text before it, parse the tag off the byte position
                // (no lexer), resume after the `}`. A `{` with no matching `}` errors in
                // `parse_expression_tag_at`, matching Svelte's reject.
                Some(b'{') => {
                    self.push_rcdata_text(&mut nodes, chunk_start, i);
                    let tag = self.parse_expression_tag_at(i)?;
                    i = tag.span.end as usize;
                    chunk_start = i;
                    nodes.push(FragmentNode::ExpressionTag(tag));
                }
                // Any other byte (including `<`) is literal RCDATA text.
                Some(_) => i += 1,
            }
        };

        let end = close_gt + 1;
        self.advance_to_position(end)?;
        Ok((nodes, end as u32))
    }

    /// Push a `Text` chunk covering `[start, end)`, skipping it when empty (Svelte's
    /// `flush`: an empty chunk emits no node). RCDATA text decodes with attribute-value
    /// rules — Svelte hardcodes `decode_character_references(raw, true)` in `read_sequence`.
    fn push_rcdata_text(
        &self,
        nodes: &mut BumpVec<'arena, FragmentNode<'arena>>,
        start: usize,
        end: usize,
    ) {
        if end > start {
            let span = Span {
                start: start as u32,
                end: end as u32,
            };
            nodes.push(FragmentNode::Text(Text::new(
                span,
                TextDecoding::AttributeValue,
                span,
                self.source,
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_component;

    #[test]
    fn component_classification() {
        // Uppercase first char ⇒ component.
        assert!(is_component("Comp"));
        assert!(is_component("ns.Comp"));
        assert!(is_component("deep.nested.Comp"));
        // Any dotted tag is member access ⇒ component, regardless of segment casing.
        assert!(is_component("Object.component"));
        assert!(is_component("object.property"));
        assert!(is_component("ns.lower"));
        // No dot + lowercase first char ⇒ regular HTML element.
        assert!(!is_component("div"));
        // Empty name has no first char.
        assert!(!is_component(""));
        // Non-ASCII uppercase still counts.
        assert!(is_component("Über"));
        assert!(!is_component("élan"));
    }
}
