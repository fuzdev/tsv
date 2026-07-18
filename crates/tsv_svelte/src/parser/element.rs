// Element parsing

use bumpalo::collections::Vec as BumpVec;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;
use super::{find_exact_tag_close, rcdata_close_at};

/// Whether `name` is an acceptable Svelte element or component tag name, mirroring Svelte's
/// accept gate (`1-parse/state/element.js`): `is_valid_element_name(name) ||
/// regex_valid_component_name.test(name)`. The component half reuses the shared
/// [`is_component_name`] predicate — `char::is_uppercase`, a superset of Svelte's `\p{Lu}`
/// over the *valid* set — so a non-ASCII-initial name is admissible only when it is a
/// component (`<Δfoo>`, `<ns.Comp>`), never a plain non-ASCII element (`<élan>`, `<你好>`).
/// tsv's tag lexer reads a raw name run, so without this whole-name check it over-accepts
/// names the HTML tokenizer (start tag only on `<` + ASCII alpha) and Svelte both reject.
fn is_valid_tag_name(name: &str) -> bool {
    is_valid_element_name(name) || is_component_name(name)
}

/// Port of Svelte's `is_valid_element_name` (`1-parse/state/element.js`): a doctype
/// (`<!DOCTYPE>`), a namespaced meta/element name (`<svelte:head>`, `<foo:bar>`), or a valid
/// HTML/SVG/MathML/custom element name (`REGEX_VALID_TAG_NAME`).
fn is_valid_element_name(name: &str) -> bool {
    is_doctype_name(name) || is_namespaced_name(name) || is_valid_element_local_name(name)
}

/// `regex_doctype_name` = `/^![a-zA-Z]+$/` — `!` then one or more ASCII letters.
fn is_doctype_name(name: &str) -> bool {
    name.strip_prefix('!')
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphabetic()))
}

/// `regex_namespaced_name` = `/^[a-zA-Z][a-zA-Z0-9]*:[a-zA-Z][a-zA-Z0-9-]*[a-zA-Z0-9]$/` — a
/// single ASCII colon; the prefix is `[a-zA-Z][a-zA-Z0-9]*`, the local part is ≥2 ASCII chars
/// starting with a letter and ending alphanumeric (interior `-` allowed). Covers `svelte:*`
/// meta tags and namespaced regular elements (`foo:bar`).
fn is_namespaced_name(name: &str) -> bool {
    let Some((prefix, local)) = name.split_once(':') else {
        return false;
    };
    let prefix = prefix.as_bytes();
    if !prefix.first().is_some_and(u8::is_ascii_alphabetic)
        || !prefix[1..].iter().all(u8::is_ascii_alphanumeric)
    {
        return false;
    }
    let local = local.as_bytes();
    local.len() >= 2
        && local[0].is_ascii_alphabetic()
        && local[local.len() - 1].is_ascii_alphanumeric()
        && local[1..local.len() - 1]
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'-')
}

/// `REGEX_VALID_TAG_NAME` (`utils.js`): `/^[a-zA-Z][a-zA-Z0-9]*(-[…PCENChar…]*)?$/u` — an
/// ASCII-alpha start, ASCII alphanumerics, then optionally a hyphen introducing a run of
/// [`PCENChar`](tsv_html::is_pcen_char) (custom-element names such as `<my-café>`). The Unicode
/// ranges are literal, so no general-category lookup is needed.
fn is_valid_element_local_name(name: &str) -> bool {
    let mut chars = name.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    let mut after_hyphen = false;
    for c in chars {
        if after_hyphen {
            if !tsv_html::is_pcen_char(c) {
                return false;
            }
        } else if c.is_ascii_alphanumeric() {
            // still in the leading `[a-zA-Z0-9]*` run
        } else if c == '-' {
            after_hyphen = true; // the hyphen that opens the PCENChar tail
        } else {
            return false;
        }
    }
    true
}

/// Result type for parsing elements - either a regular element or a special element.
pub(crate) enum ParsedElement<'arena> {
    Element(Element<'arena>),
    SpecialElement(SpecialElement<'arena>),
}

/// Result of parsing special element attributes: the attribute list with any `this` lifted
/// out of it, plus that `this` binding for whichever of the two tags carries one.
///
/// Two slots rather than one because the tags accept different forms — `<svelte:element>`
/// takes either (`this="div"` and `this={tag}` are both legal), `<svelte:component>` only
/// the braced one, a non-`{expression}` being rejected as it is parsed. So what survives
/// for the component is always an `ExpressionTag`, and the two slots' types differ: they
/// cannot be transposed at the call site, and only the one matching `tag` is ever `Some`.
type SpecialElementAttrs<'arena> = (
    BumpVec<'arena, AttributeNode<'arena>>,
    Option<SpecialThis<'arena>>,
    Option<ExpressionTag<'arena>>,
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

        // The tag name must immediately follow `<` and be a valid element/component name;
        // `svelte.parse` rejects `< div>` (whitespace after `<`, which tsv's tag lexer skips),
        // `<1>` / `<_x>` / `<$x>` (invalid start char), and `<élan>` / `<divä>` (non-ASCII
        // outside a valid custom-element/component name) at parse.
        if name_span.start as usize != start + 1 || !is_valid_tag_name(tag_name) {
            return Err(self.error_msg_at("Expected a valid element or component name", start + 1));
        }

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
        let kind = if is_component_name(tag_name) {
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
            facts: TagFacts::compute(tag_name),
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

        // Construct the final SpecialElementKind, rejecting a `this`-less element/component
        // the way Svelte's parser does.
        let kind = self.build_special_element_kind(tag, tag_expr, component_expr, name_span)?;

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

    /// Build the final SpecialElementKind from the tag and the extracted `this` binding.
    ///
    /// The two tags that take a `this` require one: Svelte's parser rejects the element
    /// without it (`svelte_element_missing_this`) and the component likewise
    /// (`svelte_component_missing_this`), and tsv is a drop-in for that parser. Rejecting
    /// here rather than fabricating a placeholder is load-bearing, not pedantry — the
    /// invented binding used to reach the printer and be emitted as source the author never
    /// wrote (a spurious `this=""` / `this={null}`), and the element's zero-span placeholder
    /// literal panicked when formatted (`format_string_literal_from_ast` slicing `""[1..len-1]`).
    ///
    /// The component's *other* rejection — a `this` that is not an `{expression}` — belongs
    /// to [`Self::parse_special_element_attributes`] instead, which is where the difference
    /// between "no `this` at all" and "a `this` we cannot use" is still visible.
    fn build_special_element_kind(
        &self,
        tag: SpecialElementTag,
        tag_expr: Option<SpecialThis<'arena>>,
        component_expr: Option<ExpressionTag<'arena>>,
        name_span: Span,
    ) -> Result<SpecialElementKind<'arena>, ParseError> {
        Ok(match tag {
            SpecialElementTag::SvelteElement => {
                let Some(tag) = tag_expr else {
                    return Err(self.error_msg_at(
                        "`<svelte:element>` must have a 'this' attribute with a value",
                        name_span.start as usize,
                    ));
                };
                SpecialElementKind::SvelteElement { tag }
            }
            SpecialElementTag::SvelteComponent => {
                let Some(expression) = component_expr else {
                    return Err(self.error_msg_at(
                        "`<svelte:component>` must have a 'this' attribute",
                        name_span.start as usize,
                    ));
                };
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
        })
    }

    /// Parse attributes for a special element, extracting `this` for svelte:element and svelte:component
    fn parse_special_element_attributes(
        &mut self,
        tag: SpecialElementTag,
    ) -> Result<SpecialElementAttrs<'arena>, ParseError> {
        let mut attributes = self.bvec();
        let mut tag_expr: Option<SpecialThis<'arena>> = None;
        let mut component_expr: Option<ExpressionTag<'arena>> = None;

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
                                    // Keep the whole tag, not just its expression: the `{…}`
                                    // span is where the printer looks for comments.
                                    tag_expr = Some(SpecialThis::Braced(et.clone()));
                                    continue; // Don't add to attributes
                                } else if let Some(AttributeValue::Text(t)) = values.first() {
                                    // String value: no expression is parsed, so keep the
                                    // decoded text itself. It is copied once into the arena
                                    // (the source slice carries entities and no quotes, so
                                    // it is not a verbatim slice of it).
                                    tag_expr = Some(SpecialThis::Plain {
                                        content: self.alloc_str_in(&t.data(self.source)),
                                        span: t.span,
                                    });
                                    continue;
                                }
                            }
                        } else if tag == SpecialElementTag::SvelteComponent {
                            // Svelte's `is_expression_attribute`: exactly one chunk, and an
                            // `{expression}`. A bare `this`, a string, or a multi-chunk
                            // value (`this="a{b}"`, `this={a}{b}`) is rejected outright —
                            // where `<svelte:element>` above merely warns and keeps the
                            // first chunk, a Svelte 4 behaviour it preserves on purpose.
                            let Some([AttributeValue::ExpressionTag(et)]) = a.value else {
                                return Err(self.error_msg_at(
                                    "Invalid component definition — must be an `{expression}`",
                                    a.span.start as usize,
                                ));
                            };
                            // Keep the whole tag — see the `svelte:element` arm above.
                            component_expr = Some(et.clone());
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

        // Reposition the lexer to the closing tag. We resume AT the `<`, which lexes to
        // `LeftAngle` in either mode, so the (stale, content-dependent) `inside_tag` here
        // doesn't matter — `parse_closing_tag` consumes the close and its `>` returns the
        // lexer to template mode. Contrast `parse_rcdata_content`, which resumes PAST the
        // close's `>` and so must force template mode itself.
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
        // After `</textarea>` we're back in template mode, but `inside_tag` is stale: the
        // manual scan above jumped the cursor forward, so it still reflects the token the
        // lexer stopped on when the opening `>` was consumed — the close's `<` for
        // empty/`<`-first content, which set tag mode (`{`-first content leaves it false,
        // so the pre-fix bug was content-dependent). Left as-is, `advance_to_position`
        // preserves that stale tag mode and a bare-text sibling (`</textarea>x`) lexes `x`
        // as an Identifier the markup loop rejects (`{expr}`/`<el>` siblings survive —
        // `{`/`<` are special in both modes). Force template mode before resuming.
        self.lexer.inside_tag = false;
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
    use super::{is_namespaced_name, is_valid_element_name, is_valid_tag_name};
    use crate::ast::internal::is_component_name;

    #[test]
    fn component_classification() {
        // Uppercase first char ⇒ component.
        assert!(is_component_name("Comp"));
        assert!(is_component_name("ns.Comp"));
        assert!(is_component_name("deep.nested.Comp"));
        // Any dotted tag is member access ⇒ component, regardless of segment casing.
        assert!(is_component_name("Object.component"));
        assert!(is_component_name("object.property"));
        assert!(is_component_name("ns.lower"));
        // No dot + lowercase first char ⇒ regular HTML element.
        assert!(!is_component_name("div"));
        // Empty name has no first char.
        assert!(!is_component_name(""));
        // Non-ASCII uppercase still counts (the printer must agree — see TagFacts).
        assert!(is_component_name("Über"));
        assert!(!is_component_name("élan"));
        // A `:`-namespaced tag is a RegularElement, never a component — even with an
        // uppercase prefix (Svelte's `regex_valid_component_name` rejects `:`).
        assert!(!is_component_name("foo:bar"));
        assert!(!is_component_name("Foo:bar"));
        assert!(!is_component_name("svelte:head"));
    }

    #[test]
    fn accepts_valid_element_and_component_names() {
        // Standard HTML/SVG elements and ASCII custom elements.
        for name in [
            "div",
            "h1",
            "a",
            "my-elem",
            "foreignObject",
            "annotation-xml",
        ] {
            assert!(is_valid_tag_name(name), "should accept <{name}>");
        }
        // Custom-element names with PCENChar after the hyphen (non-ASCII, `.`, `_`).
        for name in ["my-café", "x-\u{00B7}", "a-b.c", "a-b_c"] {
            assert!(is_valid_tag_name(name), "should accept <{name}>");
        }
        // Components: uppercase-initial (incl. non-ASCII \p{Lu}) and dotted member access.
        for name in ["Foo", "MyComp", "Δfoo", "ns.Comp", "deep.nested.Comp"] {
            assert!(is_valid_tag_name(name), "should accept <{name}>");
        }
        // Doctype and namespaced/meta tags.
        for name in [
            "!DOCTYPE",
            "!doctype",
            "svelte:head",
            "svelte:component",
            "foo:bar",
        ] {
            assert!(is_valid_tag_name(name), "should accept <{name}>");
        }
    }

    #[test]
    fn rejects_invalid_tag_names() {
        // Non-ASCII start that is not a component (lowercase / caseless / titlecase). A
        // titlecase letter (`ǅ`, category Lt) is not `\p{Lu}` nor `Uppercase`, so it is not a
        // component name — matching Svelte.
        for name in ["élan", "δfoo", "你好", "ǅfoo"] {
            assert!(!is_valid_tag_name(name), "should reject <{name}>");
        }
        // ASCII start but a non-ASCII letter mid-name without a hyphen (not PCENChar-eligible).
        assert!(!is_valid_tag_name("divä"));
        // Stray delimiters the lexer folds into the name run.
        assert!(!is_valid_tag_name("a|b"));
        // Namespaced local part too short / hyphen-terminated / non-ASCII.
        for name in ["foo:b", "foo:bar-", "foo:bär"] {
            assert!(!is_namespaced_name(name), "namespaced should reject {name}");
            assert!(
                !is_valid_element_name(name),
                "element name should reject {name}"
            );
        }
        // Doctype must be `!` + ASCII letters only.
        assert!(!is_valid_element_name("!-"));
        assert!(!is_valid_element_name("!doc7"));
    }
}
