// Doc-based formatting for Svelte special elements
//
// Handles svelte:* elements:
// - svelte:component, svelte:element, svelte:self
// - svelte:window, svelte:body, svelte:document, svelte:head
// - svelte:fragment, svelte:boundary
// - slot, title

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;
use crate::printer::text::TextAnalysis;
use tsv_lang::doc::{DocBuf, arena::DocId};

impl<'a> Printer<'a> {
    /// Build a doc for a special element (svelte:component, svelte:element, etc.)
    ///
    /// Handles all special element formatting modes:
    /// - Self-closing: `<svelte:component this={C} />`
    /// - Empty: `<slot></slot>`
    /// - Inline children: `<slot name="x">text</slot>`
    /// - Hug mode: attrs fit, children force break
    /// - Full multiline: attrs wrapped, children on new lines
    pub(crate) fn build_special_element_doc(&self, element: &internal::SpecialElement) -> DocId {
        let d = self.d();
        use internal::SpecialElementKind;

        let tag_name = element.kind.tag_name();

        // Determine element characteristics
        let is_typically_empty = matches!(
            element.kind,
            SpecialElementKind::SvelteWindow
                | SpecialElementKind::SvelteBody
                | SpecialElementKind::SvelteDocument
                | SpecialElementKind::SvelteHead
                | SpecialElementKind::SvelteComponent { .. }
                | SpecialElementKind::SvelteElement { .. }
                | SpecialElementKind::SvelteSelf
                | SpecialElementKind::SlotElement
                | SpecialElementKind::SvelteFragment
                | SpecialElementKind::SvelteBoundary
                | SpecialElementKind::TitleElement
        );

        let is_inline_element = matches!(
            element.kind,
            SpecialElementKind::SlotElement
                | SpecialElementKind::SvelteFragment
                | SpecialElementKind::SvelteSelf
                | SpecialElementKind::TitleElement
        );

        let has_snippets = element
            .fragment
            .nodes
            .iter()
            .any(|n| matches!(n, FragmentNode::SnippetBlock(_)));
        let is_boundary_without_snippets =
            matches!(element.kind, SpecialElementKind::SvelteBoundary) && !has_snippets;
        let is_boundary_with_snippets =
            matches!(element.kind, SpecialElementKind::SvelteBoundary) && has_snippets;

        // Self-closing detection
        let is_self_closing = element.fragment.nodes.is_empty()
            && is_typically_empty
            && self.span_was_self_closing(element.span);

        // Build attribute docs (including this={...} for component/element)
        let attr_docs = self.build_special_element_attrs_doc(element, self.d().line());
        let has_attrs = !attr_docs.is_empty();

        // Check if any attribute doc will break (e.g., multiline string value)
        let has_multiline = attr_docs.iter().any(|&doc| d.will_break(doc));

        // Handle self-closing elements
        if is_self_closing {
            return if !has_attrs {
                d.concat(&[d.text("<"), d.text(tag_name), d.text(" />")])
            } else {
                // Self-closing with attrs - use group for proper wrapping
                let attr_concat = d.concat(&attr_docs);
                let inner = d.concat(&[
                    d.text("<"),
                    d.text(tag_name),
                    d.indent(attr_concat),
                    d.line(),
                    d.text("/>"),
                ]);

                if has_multiline {
                    d.group_break(inner)
                } else {
                    d.group(inner)
                }
            };
        }

        // Handle empty elements (not self-closing)
        if element.fragment.nodes.is_empty() {
            return if !has_attrs {
                d.concat(&[
                    d.text("<"),
                    d.text(tag_name),
                    d.text("></"),
                    d.text(tag_name),
                    d.text(">"),
                ])
            } else {
                // Empty with attrs - use conditional_group for hug mode
                let closing = d.concat(&[d.text("></"), d.text(tag_name), d.text(">")]);

                // State 1: All inline
                let attr_concat_inline = d.concat(&attr_docs);
                let inline_state = d.concat(&[
                    d.text("<"),
                    d.text(tag_name),
                    d.indent(attr_concat_inline),
                    closing,
                ]);

                // State 2: Hug mode - attrs inline (space-separated), > on new line
                let hug_attrs = self.build_special_element_attrs_doc(element, self.d().text(" "));
                let hug_attrs_concat = d.concat(&hug_attrs);
                let hug_state = d.concat(&[
                    d.text("<"),
                    d.text(tag_name),
                    hug_attrs_concat,
                    d.hardline(),
                    closing,
                ]);

                // State 3: Full multiline - attrs on separate lines
                let attr_concat_multiline = d.concat(&attr_docs);
                let multiline_state = d.concat(&[
                    d.text("<"),
                    d.text(tag_name),
                    d.indent(attr_concat_multiline),
                    d.hardline(),
                    closing,
                ]);

                d.conditional_group(&[inline_state, hug_state, multiline_state])
            };
        }

        // Element with children - determine formatting mode
        let has_block_children = element.fragment.nodes.iter().any(|n| {
            matches!(n, FragmentNode::Element(el) if self.is_block_element(el))
                || matches!(n, FragmentNode::SnippetBlock(_))
                || matches!(n, FragmentNode::SpecialElement(se)
                    if matches!(se.kind, SpecialElementKind::SvelteHead))
        });

        // Simple inline content check
        let is_simple_content = is_boundary_without_snippets
            || (!has_block_children
                && (is_inline_element
                    || element.fragment.nodes.iter().all(|n| match n {
                        FragmentNode::Text(_) | FragmentNode::ExpressionTag(_) => true,
                        FragmentNode::SpecialElement(se) => matches!(
                            se.kind,
                            SpecialElementKind::SlotElement | SpecialElementKind::SvelteFragment
                        ),
                        _ => false,
                    })));

        // Check for source multiline layout
        let source_has_leading_break = element
            .fragment
            .nodes
            .first()
            .is_some_and(FragmentNode::is_boundary_break);
        let source_has_trailing_break = element
            .fragment
            .nodes
            .last()
            .is_some_and(FragmentNode::is_boundary_break);

        // Check for expanding control flow blocks (if/each/key) or expanding blocks inside await
        // These force hug mode (not full multiline) for inline special elements like <slot>
        // BUT only when there's no whitespace around the blocks
        // NOTE: svelte:boundary NEVER uses hug mode - it always uses normal multiline for expanding blocks
        let has_expanding_blocks =
            super::helpers::has_any_expanding_blocks(&element.fragment.nodes);
        // Check if there's whitespace around expanding blocks
        // e.g., `<slot> {#if c}...{/if} </slot>` has whitespace, should use normal multiline
        let has_ws_around_expanding = has_expanding_blocks
            && element
                .fragment
                .nodes
                .iter()
                .any(|n| matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only() && !t.raw.is_empty()));
        // Only force hug mode when expanding blocks present WITHOUT surrounding whitespace
        // svelte:boundary never uses hug mode - it always uses normal multiline for expanding blocks
        let forces_hug_mode =
            !is_boundary_without_snippets && has_expanding_blocks && !has_ws_around_expanding;

        // Determine if multiline formatting is needed
        // svelte:boundary with expanding blocks always uses multiline (not hug mode)
        // Also need multiline when whitespace surrounds expanding blocks
        let boundary_needs_multiline_for_blocks =
            is_boundary_without_snippets && has_expanding_blocks;
        let needs_multiline = boundary_needs_multiline_for_blocks
            || (!is_boundary_without_snippets
                && (source_has_leading_break
                    || source_has_trailing_break
                    || (has_block_children && !is_inline_element)
                    || is_boundary_with_snippets
                    || has_ws_around_expanding));

        // Build children doc based on formatting mode
        // Special elements are block-level, so always trim boundaries
        let children_doc_tree = if needs_multiline {
            self.build_nodes_doc_multiline(&element.fragment.nodes)
        } else if is_simple_content {
            self.build_nodes_doc_trimmed(&element.fragment.nodes, true, false, false)
        } else {
            self.build_fragment_doc(&element.fragment)
        };
        let children_doc = children_doc_tree;

        // Hug mode detection (like regular elements)
        // svelte:boundary with snippets disables hug mode
        // Expanding blocks (if/each/key or those inside await) force hug mode
        let hug_start = !needs_multiline
            && !is_boundary_with_snippets
            && (forces_hug_mode || self.should_hug_start_special(element));
        let hug_end = !needs_multiline
            && !is_boundary_with_snippets
            && (forces_hug_mode || self.should_hug_end_special(element));

        // Build the final doc based on hug mode and attrs
        if !has_attrs {
            // No attrs - simpler structure
            if needs_multiline {
                let inner = d.concat(&[d.hardline(), children_doc]);
                d.concat(&[
                    d.text("<"),
                    d.text(tag_name),
                    d.text(">"),
                    d.indent(inner),
                    d.hardline(),
                    d.text("</"),
                    d.text(tag_name),
                    d.text(">"),
                ])
            } else if forces_hug_mode {
                // Expanding blocks force hug mode with hardlines
                // <slot
                //   >{#if c}text{/if}</slot
                // >
                let inner_group =
                    d.concat(&[d.text(">"), children_doc, d.text("</"), d.text(tag_name)]);
                let inner_group = d.group(inner_group);
                let indent_content = d.concat(&[d.hardline(), inner_group]);
                d.group(d.concat(&[
                    d.text("<"),
                    d.text(tag_name),
                    d.indent(indent_content),
                    d.hardline(),
                    d.text(">"),
                ]))
            } else if hug_start && hug_end {
                // Hug both - use group with softlines
                let inner_group =
                    d.concat(&[d.text(">"), children_doc, d.text("</"), d.text(tag_name)]);
                let inner_group = d.group(inner_group);
                let indent_content = d.concat(&[d.softline(), inner_group]);
                d.group(d.concat(&[
                    d.text("<"),
                    d.text(tag_name),
                    d.indent(indent_content),
                    d.softline(),
                    d.text(">"),
                ]))
            } else {
                // Inline
                d.concat(&[
                    d.text("<"),
                    d.text(tag_name),
                    d.text(">"),
                    children_doc,
                    d.text("</"),
                    d.text(tag_name),
                    d.text(">"),
                ])
            }
        } else if needs_multiline {
            // With attrs, multiline children
            let attr_concat = d.concat(&attr_docs);
            let sl = d.softline();
            let trailing = d.dedent(sl);
            let attr_inner = d.concat(&[attr_concat, trailing]);
            let attr_group = d.group(attr_inner);
            let attr_indent = d.indent(attr_group);
            let inner = d.concat(&[d.hardline(), children_doc]);
            d.concat(&[
                d.text("<"),
                d.text(tag_name),
                attr_indent,
                d.text(">"),
                d.indent(inner),
                d.hardline(),
                d.text("</"),
                d.text(tag_name),
                d.text(">"),
            ])
        } else if forces_hug_mode {
            // Expanding blocks force hug mode with hardlines (with attrs)
            // <slot name="x"
            //   >{#if c}text{/if}</slot
            // >
            let body = d.concat(&[d.text(">"), children_doc, d.text("</"), d.text(tag_name)]);

            let attr_concat = d.concat(&attr_docs);
            let body_indent = d.concat(&[d.hardline(), body]);
            d.group(d.concat(&[
                d.text("<"),
                d.text(tag_name),
                d.indent(d.group(attr_concat)),
                d.indent(body_indent),
                d.hardline(),
                d.text(">"),
            ]))
        } else if hug_start && hug_end {
            // With attrs, hug mode - use nested groups like regular elements
            // Outer group: controls whether content goes on new line
            // Inner group (around attrs): controls whether attrs break
            let body = d.concat(&[d.text(">"), children_doc, d.text("</"), d.text(tag_name)]);

            let attr_concat = d.concat(&attr_docs);
            let attr_group = if has_multiline {
                d.group_break(attr_concat)
            } else {
                d.group(attr_concat)
            };

            let body_indent_softline = d.indent_softline(body);
            let inner = d.concat(&[
                d.text("<"),
                d.text(tag_name),
                d.indent(attr_group),
                d.group(body_indent_softline),
                d.softline(),
                d.text(">"),
            ]);

            if has_multiline {
                d.group_break(inner)
            } else {
                d.group(inner)
            }
        } else if has_multiline {
            // With attrs containing multiline value, inline children
            // Force attrs to break and use hug structure like Prettier
            let body = d.concat(&[d.text(">"), children_doc, d.text("</"), d.text(tag_name)]);

            let attr_concat = d.concat(&attr_docs);
            let body_indent_softline = d.indent_softline(body);
            d.group_break(d.concat(&[
                d.text("<"),
                d.text(tag_name),
                d.indent(attr_concat),
                d.group(body_indent_softline),
                d.softline(),
                d.text(">"),
            ]))
        } else {
            // With attrs, inline children
            let attr_concat = d.concat(&attr_docs);
            d.group(d.concat(&[
                d.text("<"),
                d.text(tag_name),
                d.indent(attr_concat),
                d.text(">"),
                children_doc,
                d.text("</"),
                d.text(tag_name),
                d.text(">"),
            ]))
        }
    }

    /// Build docs for special element attributes.
    ///
    /// `separator`: emitted between attributes — `d.line()` for the wrapping
    /// (line-separated) layout, `d.text(" ")` for hug mode (space-separated).
    pub(crate) fn build_special_element_attrs_doc(
        &self,
        element: &internal::SpecialElement,
        separator: DocId,
    ) -> DocBuf {
        // Pre-allocate: 2 docs per attr (separator + attr), plus potential this={} attr
        let has_this = matches!(
            element.kind,
            internal::SpecialElementKind::SvelteComponent { .. }
                | internal::SpecialElementKind::SvelteElement { .. }
        );
        let capacity = (element.attributes.len() + usize::from(has_this)) * 2;
        let mut docs: DocBuf = DocBuf::with_capacity(capacity);

        // Add this={...} for component/element
        match &element.kind {
            internal::SpecialElementKind::SvelteComponent { expression } => {
                docs.push(separator);
                docs.push(self.build_this_attr_doc_for_inline(expression));
            }
            internal::SpecialElementKind::SvelteElement { tag } => {
                docs.push(separator);
                docs.push(self.build_this_attr_doc_for_inline(tag));
            }
            _ => {}
        }

        // svelte:element renders as HTML, so normalize class attribute whitespace
        let normalize_class = matches!(
            element.kind,
            internal::SpecialElementKind::SvelteElement { .. }
        );
        self.push_attrs_with_comments(
            &mut docs,
            &element.attributes,
            separator,
            element.name_span.end,
            element.open_tag_end,
            normalize_class,
        );

        docs
    }

    /// Build doc for this={expression} attribute (for inline doc building)
    fn build_this_attr_doc_for_inline(&self, expr: &tsv_ts::Expression) -> DocId {
        let d = self.d();
        use tsv_ts::ast::internal::{Expression, LiteralValue};

        // Handle plain string attribute: this="value" (no braces in source)
        // Distinguished from expression form this={"value"} by checking source:
        // - Plain string: span covers `hello` (no quote at span start)
        // - Expression: span covers `"hello"` (quote char at span start)
        if let Expression::Literal(lit) = expr
            && let LiteralValue::String { content, .. } = &lit.value
        {
            let first_byte = self.source.as_bytes().get(lit.span.start as usize).copied();
            if first_byte != Some(b'"') && first_byte != Some(b'\'') {
                return d.text_owned(format!("this=\"{content}\""));
            }
        }

        // Expression (including braced string literals): this={expr}
        let expr_doc_id = self.build_ts_expression_doc_no_comments(expr);
        d.concat(&[d.text("this={"), expr_doc_id, d.text("}")])
    }

    /// Check if special element should hug the start (no leading whitespace)
    fn should_hug_start_special(&self, element: &internal::SpecialElement) -> bool {
        if element.fragment.nodes.is_empty() {
            return true;
        }
        match &element.fragment.nodes[0] {
            FragmentNode::Text(text) => !text.raw.starts_with(char::is_whitespace),
            _ => true,
        }
    }

    /// Check if special element should hug the end (no trailing whitespace)
    fn should_hug_end_special(&self, element: &internal::SpecialElement) -> bool {
        if element.fragment.nodes.is_empty() {
            return true;
        }
        match element.fragment.nodes.last() {
            Some(FragmentNode::Text(text)) => !text.raw.ends_with(char::is_whitespace),
            _ => true,
        }
    }
}
