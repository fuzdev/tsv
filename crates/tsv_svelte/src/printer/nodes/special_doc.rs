// Doc-based formatting for Svelte special elements
//
// Handles svelte:* elements:
// - svelte:component, svelte:element, svelte:self
// - svelte:window, svelte:body, svelte:document, svelte:head
// - svelte:fragment, svelte:boundary
// - slot, title

use std::rc::Rc;

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;
use crate::printer::helpers::has_multiline_template_literal;
use crate::printer::text::TextAnalysis;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Build a doc for a special element (svelte:component, svelte:element, etc.)
    ///
    /// Handles all special element formatting modes:
    /// - Self-closing: `<svelte:component this={C} />`
    /// - Empty: `<slot></slot>`
    /// - Inline children: `<slot name="x">text</slot>`
    /// - Hug mode: attrs fit, children force break
    /// - Full multiline: attrs wrapped, children on new lines
    pub(crate) fn build_special_element_doc(&self, element: &internal::SpecialElement) -> Doc {
        use internal::SpecialElementKind;

        let tag_name = element.kind.tag_name();

        // Determine element characteristics
        let is_typically_empty = matches!(
            element.kind,
            SpecialElementKind::SvelteWindow
                | SpecialElementKind::SvelteBody
                | SpecialElementKind::SvelteDocument
                | SpecialElementKind::SvelteComponent { .. }
                | SpecialElementKind::SvelteElement { .. }
                | SpecialElementKind::SvelteSelf
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
            && self.was_special_self_closing_doc(element);

        // Build attribute docs (including this={...} for component/element)
        let attr_docs = self.build_special_element_attrs_doc(element);
        let has_attrs = !attr_docs.is_empty();

        // Check if any attribute contains a template literal with embedded newlines
        let has_multiline_template = element
            .attributes
            .iter()
            .any(|a| has_multiline_template_literal(a.span().extract(self.source)));

        // Handle self-closing elements
        if is_self_closing {
            return if !has_attrs {
                doc::concat(vec![doc::text("<"), doc::text(tag_name), doc::text(" />")])
            } else {
                // Self-closing with attrs - use group for proper wrapping
                let inner = doc::concat(vec![
                    doc::text("<"),
                    doc::text(tag_name),
                    doc::indent(doc::concat(attr_docs)),
                    doc::line(),
                    doc::text("/>"),
                ]);

                if has_multiline_template {
                    doc::group_break(inner)
                } else {
                    doc::group(inner)
                }
            };
        }

        // Handle empty elements (not self-closing)
        if element.fragment.nodes.is_empty() {
            return if !has_attrs {
                doc::concat(vec![
                    doc::text("<"),
                    doc::text(tag_name),
                    doc::text("></"),
                    doc::text(tag_name),
                    doc::text(">"),
                ])
            } else {
                // Empty with attrs - use conditional_group for hug mode
                let closing =
                    doc::concat(vec![doc::text("></"), doc::text(tag_name), doc::text(">")]);

                // State 1: All inline
                let inline_state = doc::concat(vec![
                    doc::text("<"),
                    doc::text(tag_name),
                    doc::indent(doc::concat(attr_docs.clone())),
                    closing.clone(),
                ]);

                // State 2: Hug mode - attrs inline (space-separated), > on new line
                let hug_attrs = self.build_special_element_attrs_doc_spaces(element);
                let hug_state = doc::concat(vec![
                    doc::text("<"),
                    doc::text(tag_name),
                    doc::concat(hug_attrs),
                    doc::hardline(),
                    closing.clone(),
                ]);

                // State 3: Full multiline - attrs on separate lines
                let multiline_state = doc::concat(vec![
                    doc::text("<"),
                    doc::text(tag_name),
                    doc::indent(doc::concat(attr_docs)),
                    doc::hardline(),
                    closing,
                ]);

                doc::conditional_group(vec![inline_state, hug_state, multiline_state])
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
                    || has_block_children
                    || is_boundary_with_snippets
                    || has_ws_around_expanding));

        // Build children doc based on formatting mode
        // Special elements are block-level, so always trim boundaries
        let children_doc = if needs_multiline {
            self.build_nodes_doc_multiline(&element.fragment.nodes)
        } else if is_simple_content {
            self.build_nodes_doc_trimmed(&element.fragment.nodes, true)
        } else {
            self.build_fragment_doc(&element.fragment)
        };

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
                doc::concat(vec![
                    doc::text("<"),
                    doc::text(tag_name),
                    doc::text(">"),
                    doc::indent(doc::concat(vec![doc::hardline(), children_doc])),
                    doc::hardline(),
                    doc::text("</"),
                    doc::text(tag_name),
                    doc::text(">"),
                ])
            } else if forces_hug_mode {
                // Expanding blocks force hug mode with hardlines
                // <slot
                //   >{#if c}text{/if}</slot
                // >
                doc::group(doc::concat(vec![
                    doc::text("<"),
                    doc::text(tag_name),
                    doc::indent(doc::concat(vec![
                        doc::hardline(),
                        doc::group(doc::concat(vec![
                            doc::text(">"),
                            children_doc,
                            doc::text("</"),
                            doc::text(tag_name),
                        ])),
                    ])),
                    doc::hardline(),
                    doc::text(">"),
                ]))
            } else if hug_start && hug_end {
                // Hug both - use group with softlines
                doc::group(doc::concat(vec![
                    doc::text("<"),
                    doc::text(tag_name),
                    doc::indent(doc::concat(vec![
                        doc::softline(),
                        doc::group(doc::concat(vec![
                            doc::text(">"),
                            children_doc,
                            doc::text("</"),
                            doc::text(tag_name),
                        ])),
                    ])),
                    doc::softline(),
                    doc::text(">"),
                ]))
            } else {
                // Inline
                doc::concat(vec![
                    doc::text("<"),
                    doc::text(tag_name),
                    doc::text(">"),
                    children_doc,
                    doc::text("</"),
                    doc::text(tag_name),
                    doc::text(">"),
                ])
            }
        } else if needs_multiline {
            // With attrs, multiline children
            doc::concat(vec![
                doc::text("<"),
                doc::text(tag_name),
                doc::group(doc::indent(doc::concat(attr_docs))),
                doc::text(">"),
                doc::indent(doc::concat(vec![doc::hardline(), children_doc])),
                doc::hardline(),
                doc::text("</"),
                doc::text(tag_name),
                doc::text(">"),
            ])
        } else if forces_hug_mode {
            // Expanding blocks force hug mode with hardlines (with attrs)
            // <slot name="x"
            //   >{#if c}text{/if}</slot
            // >
            let body = doc::concat(vec![
                doc::text(">"),
                children_doc,
                doc::text("</"),
                doc::text(tag_name),
            ]);

            doc::group(doc::concat(vec![
                doc::text("<"),
                doc::text(tag_name),
                doc::indent(doc::group(doc::concat(attr_docs))),
                doc::indent(doc::concat(vec![doc::hardline(), body])),
                doc::hardline(),
                doc::text(">"),
            ]))
        } else if hug_start && hug_end {
            // With attrs, hug mode - use nested groups like regular elements
            // Outer group: controls whether content goes on new line
            // Inner group (around attrs): controls whether attrs break
            let body = doc::concat(vec![
                doc::text(">"),
                children_doc,
                doc::text("</"),
                doc::text(tag_name),
            ]);

            let attr_group = if has_multiline_template {
                doc::group_break(doc::concat(attr_docs))
            } else {
                doc::group(doc::concat(attr_docs))
            };

            let inner = doc::concat(vec![
                doc::text("<"),
                doc::text(tag_name),
                doc::indent(attr_group),
                doc::group(doc::indent_softline(body)),
                doc::softline(),
                doc::text(">"),
            ]);

            if has_multiline_template {
                doc::group_break(inner)
            } else {
                doc::group(inner)
            }
        } else if has_multiline_template {
            // With attrs containing multiline template, inline children
            // Force attrs to break and use hug structure like Prettier
            let body = doc::concat(vec![
                doc::text(">"),
                children_doc,
                doc::text("</"),
                doc::text(tag_name),
            ]);

            doc::group_break(doc::concat(vec![
                doc::text("<"),
                doc::text(tag_name),
                doc::indent(doc::concat(attr_docs)),
                doc::group(doc::indent_softline(body)),
                doc::softline(),
                doc::text(">"),
            ]))
        } else {
            // With attrs, inline children
            doc::group(doc::concat(vec![
                doc::text("<"),
                doc::text(tag_name),
                doc::indent(doc::concat(attr_docs)),
                doc::text(">"),
                children_doc,
                doc::text("</"),
                doc::text(tag_name),
                doc::text(">"),
            ]))
        }
    }

    /// Build docs for special element attributes (line-separated)
    pub(crate) fn build_special_element_attrs_doc(
        &self,
        element: &internal::SpecialElement,
    ) -> Vec<Doc> {
        self.build_special_element_attrs_doc_impl(element, doc::line())
    }

    /// Build docs for special element attributes (space-separated, for hug mode)
    pub(crate) fn build_special_element_attrs_doc_spaces(
        &self,
        element: &internal::SpecialElement,
    ) -> Vec<Doc> {
        self.build_special_element_attrs_doc_impl(element, doc::text(" "))
    }

    /// Build docs for special element attributes with configurable separator
    fn build_special_element_attrs_doc_impl(
        &self,
        element: &internal::SpecialElement,
        separator: Doc,
    ) -> Vec<Doc> {
        // Pre-allocate: 2 docs per attr (separator + attr), plus potential this={} attr
        let has_this = matches!(
            element.kind,
            internal::SpecialElementKind::SvelteComponent { .. }
                | internal::SpecialElementKind::SvelteElement { .. }
        );
        let capacity = (element.attributes.len() + usize::from(has_this)) * 2;
        let mut docs = Vec::with_capacity(capacity);

        // Add this={...} for component/element
        match &element.kind {
            internal::SpecialElementKind::SvelteComponent { expression } => {
                docs.push(separator.clone());
                docs.push(self.build_this_attr_doc_for_inline(expression));
            }
            internal::SpecialElementKind::SvelteElement { tag } => {
                docs.push(separator.clone());
                docs.push(self.build_this_attr_doc_for_inline(tag));
            }
            _ => {}
        }

        // Add regular attributes
        for attr in &element.attributes {
            docs.push(separator.clone());
            docs.push(self.build_attribute_node_doc(attr));
        }

        docs
    }

    /// Build doc for this={expression} attribute (for inline doc building)
    fn build_this_attr_doc_for_inline(&self, expr: &tsv_ts::Expression) -> Doc {
        use tsv_ts::ast::internal::{Expression, LiteralValue};

        // Handle string literal: this="value" (without braces)
        if let Expression::Literal(lit) = expr
            && let LiteralValue::String { content, .. } = &lit.value
        {
            return doc::text_owned(format!("this=\"{content}\""));
        }

        // Expression: this={expr}
        let expr_doc = tsv_ts::build_expression_doc(
            expr,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            &self.line_breaks,
        );
        doc::concat(vec![doc::text("this={"), expr_doc, doc::text("}")])
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

    /// Check if special element was self-closing in source (for doc building)
    pub(super) fn was_special_self_closing_doc(&self, element: &internal::SpecialElement) -> bool {
        element.span.extract(self.source).trim_end().ends_with("/>")
    }
}
