// Doc-based formatting for regular HTML/component elements
//
// Handles all element types except svelte:* special elements:
// - HTML elements (div, span, etc.)
// - Components (PascalCase)
// - Void elements (br, img, etc.)
// - Raw content elements (script, style)
//
// Whitespace-sensitive elements (pre, textarea) are dispatched from here to the
// builders in `element_ws_sensitive_doc.rs`; the analyze/classify predicates live
// in `element_analysis.rs`. The shared types (`BoundaryMode`, `ElementLayout`,
// `ElementKind`, `ElementContext`) are defined here and used by both.

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;
use crate::printer::text::TextAnalysis;
use smallvec::smallvec;
use tsv_lang::comments_in_range;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::{Span, SymbolResolver, SymbolToU32};

/// How content relates to an element boundary (opening or closing tag)
///
/// This determines what separator (if any) appears between the tag and content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundaryMode {
    /// Content touches tag directly, no separator
    /// Example: `<span>text` or `text</span>`
    Hug,
    /// Hardline separator - preserves source structure
    /// Example: `<p>\n  text` (source had newline, preserve it)
    Hard,
    /// Softline separator - collapses or breaks based on fit
    /// Example: `<span> text` where space can collapse if needed
    Soft,
}

/// Element layout classification for doc building
///
/// Determines which doc structure to use based on element type and content.
#[derive(Debug)]
pub(super) enum ElementLayout {
    /// Void element: `<br>`, `<img>`, etc. - no closing tag
    Void,
    /// Self-closing: `<Component />` - explicit self-close
    SelfClosing,
    /// Empty element with optional softline: `<div></div>`
    Empty,
    /// Element with content and boundary modes
    WithContent {
        start: BoundaryMode,
        end: BoundaryMode,
        /// Whether children need multiline formatting (each on own line)
        multiline_children: bool,
    },
}

/// Element type classification
///
/// Determines whitespace handling and formatting behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ElementKind {
    /// Svelte component (PascalCase or namespaced like `svelte:component`)
    Component,
    /// HTML block element (div, p, section, etc.)
    Block,
    /// HTML inline element (span, a, strong, etc.)
    Inline,
}

impl ElementKind {
    pub(super) fn is_component(self) -> bool {
        matches!(self, ElementKind::Component)
    }

    pub(super) fn is_block(self) -> bool {
        matches!(self, ElementKind::Block)
    }

    pub(super) fn is_inline(self) -> bool {
        matches!(self, ElementKind::Inline)
    }

    /// Whether this element type preserves source structure at boundaries
    pub(super) fn preserves_boundary_breaks(self) -> bool {
        matches!(self, ElementKind::Block | ElementKind::Component)
    }
}

/// Analysis context for element formatting decisions
///
/// Computed once per element, used to determine layout and build docs.
/// The bools capture orthogonal properties needed by different builder methods.
#[allow(clippy::struct_excessive_bools)]
pub(super) struct ElementContext {
    /// Element type classification
    pub(super) kind: ElementKind,
    /// Whether element is void (br, img, etc.)
    pub(super) is_void: bool,
    /// Whether element was self-closing in source
    pub(super) is_self_closing: bool,
    /// Whether element has no meaningful content
    pub(super) is_empty: bool,
    /// Whether source has newline at opening boundary
    pub(super) source_has_leading_break: bool,
    /// Whether source has newline at closing boundary
    pub(super) source_has_trailing_break: bool,
    /// Whether content should hug the opening tag
    pub(super) hug_start: bool,
    /// Whether content should hug the closing tag
    pub(super) hug_end: bool,
    /// Whether children need multiline formatting
    pub(super) needs_multiline: bool,
    /// Whether block-flow children (if/each/etc.) force this element to multiline layout. Cached
    /// `has_block_flow_children && block_flow_forces_multiline` — the only combination the three
    /// readers (`force` layout, `will_go_multiline`, `compute_needs_multiline`) ever need.
    pub(super) block_flow_multiline: bool,
    /// Whether to trim boundary whitespace from children
    pub(super) trim_boundaries: bool,
    /// Whether any attribute source contains embedded newlines (forces attr group break)
    pub(super) has_multiline_attr: bool,
    /// Whether all content children are text nodes (no elements, expressions, blocks)
    pub(super) only_text_content: bool,
}

impl<'a> Printer<'a> {
    /// `<name>` — a start tag with no attributes (HTML spec "start tag").
    #[inline]
    pub(super) fn start_tag(&self, name: u32) -> DocId {
        let d = self.d();
        d.concat(&[d.text("<"), d.symbol(name), d.text(">")])
    }

    /// `</name>` — an end tag (HTML spec "end tag").
    #[inline]
    pub(super) fn end_tag(&self, name: u32) -> DocId {
        let d = self.d();
        d.concat(&[d.text("</"), d.symbol(name), d.text(">")])
    }

    /// `<name />` — a self-closing start tag with no attributes.
    #[inline]
    fn self_closing_tag(&self, name: u32) -> DocId {
        let d = self.d();
        d.concat(&[d.text("<"), d.symbol(name), d.text(" />")])
    }

    /// Build a doc for an element (regular HTML or component)
    ///
    /// Uses a three-phase approach:
    /// 1. Analyze: Compute all formatting-relevant properties
    /// 2. Classify: Determine layout strategy (void, empty, hug modes, etc.)
    /// 3. Build: Construct doc based on layout
    pub(crate) fn build_element_doc(&self, element: &internal::Element<'_>) -> DocId {
        let tag_name = self.resolve_symbol(element.name);
        let tag_sym = element.name.to_u32();
        let is_html = element.kind == internal::ElementKind::Html;

        // Build attribute docs (needed for all paths)
        let attr_docs = self.build_element_attrs_doc(
            element.attributes,
            self.d().line(),
            element.name_span.end,
            element.open_tag_end,
            is_html,
        );

        // Special handling for <style> and <script> elements
        if tag_name == "style" || tag_name == "script" {
            return self.build_raw_content_element_doc(&tag_name, element, attr_docs);
        }

        // Foreign language <template> elements (e.g., <template lang="pug">)
        // preserve content raw — we can't format non-HTML template languages
        if tag_name == "template"
            && let Some(lang) = self.get_lang_attribute(element.attributes)
            && lang != "html"
        {
            return self.build_foreign_template_doc(element);
        }

        // Whitespace-sensitive elements (pre, textarea, etc.)
        if tsv_html::preserves_whitespace(&tag_name) {
            return self.build_whitespace_sensitive_element_doc(&tag_name, element, attr_docs);
        }

        // Phase 1: Analyze element
        let ctx = self.analyze_element(element, &attr_docs);

        // Phase 2: Compute layout
        let layout = self.compute_element_layout(&ctx);

        // Phase 3: Build doc based on layout
        match layout {
            ElementLayout::Void | ElementLayout::SelfClosing => {
                // DOCTYPE uses > (no self-closing slash) — it's a declaration, not an element
                let is_declaration = tag_name.starts_with('!');
                self.build_void_element_doc(tag_sym, attr_docs, is_declaration)
            }
            ElementLayout::Empty => {
                let opening_tag = self.build_opening_tag(
                    tag_sym,
                    &attr_docs,
                    false,
                    ctx.is_empty,
                    ctx.has_multiline_attr,
                );
                self.build_empty_element_doc(element, opening_tag, !attr_docs.is_empty(), ctx.kind)
            }
            ElementLayout::WithContent {
                start,
                end,
                multiline_children,
            } => self.build_content_element_doc(
                element,
                &ctx,
                &attr_docs,
                start,
                end,
                multiline_children,
            ),
        }
    }

    /// Build an inline content element that hands its trailing closing `>` to a following
    /// sibling (the axis-3 sibling-`>` dangle). Returns `Some(doc)` ending in `</tag` (no
    /// `>`) only when the element uses the flat hug-both content layout — the single shape
    /// where splitting the `>` off is render-safe and well-defined. Returns `None`
    /// otherwise so the caller keeps the element (and its `>`) intact. The caller emits the
    /// `>` itself (see `build_expanding_construct`'s `gt_prefix`).
    pub(crate) fn build_inline_element_omit_close_gt(
        &self,
        element: &internal::Element<'_>,
    ) -> Option<DocId> {
        // Special-content elements (raw `<script>`/`<style>`, foreign `<template>`,
        // whitespace-sensitive `<pre>`/`<textarea>`) never participate — their closing
        // tags aren't the simple hug-both shape. Resolve once inside the borrow to
        // skip a per-call `String` alloc.
        let (always_skip, is_template) = self.with_resolved_symbol(element.name, |t| {
            (
                t == "style" || t == "script" || tsv_html::preserves_whitespace(t),
                t == "template",
            )
        });
        if always_skip
            || (is_template
                && self
                    .get_lang_attribute(element.attributes)
                    .is_some_and(|lang| lang != "html"))
        {
            return None;
        }
        let is_html = element.kind == internal::ElementKind::Html;
        let attr_docs = self.build_element_attrs_doc(
            element.attributes,
            self.d().line(),
            element.name_span.end,
            element.open_tag_end,
            is_html,
        );
        let ctx = self.analyze_element(element, &attr_docs);
        // Only the flat hug-both content layout has a single trailing `>` we can cleanly
        // split off. Multiline children, boundary breaks, and the void/empty/self-closing
        // and non-hug boundary forms all keep their `>` (return None → no dangle).
        match self.compute_element_layout(&ctx) {
            ElementLayout::WithContent {
                start: BoundaryMode::Hug,
                end: BoundaryMode::Hug,
                multiline_children: false,
            } => {
                let trim_text = !ctx.kind.is_inline() && ctx.only_text_content;
                let children_doc =
                    self.build_nodes_doc_with_context(element.fragment.nodes, trim_text);
                Some(self.build_hug_both_doc(element, &ctx, &attr_docs, children_doc, true))
            }
            _ => None,
        }
    }

    /// Build doc for void or self-closing element
    ///
    /// When any attribute doc will_break (e.g., multiline string value),
    /// forces attributes to break across multiple lines to match Prettier behavior.
    fn build_void_element_doc(
        &self,
        tag_sym: u32,
        attr_docs: DocBuf,
        is_declaration: bool,
    ) -> DocId {
        let d = self.d();
        // Declarations (<!DOCTYPE>) use > without self-closing slash
        if attr_docs.is_empty() {
            if is_declaration {
                self.start_tag(tag_sym)
            } else {
                self.self_closing_tag(tag_sym)
            }
        } else if is_declaration {
            let attr_concat = d.concat(&attr_docs);
            let attr_indent = d.indent(attr_concat);
            let inner = d.concat(&[
                d.text("<"),
                d.symbol(tag_sym),
                attr_indent,
                d.softline(),
                d.text(">"),
            ]);
            d.group(inner)
        } else {
            // Check if any attribute doc will break (contains hardline)
            let has_multiline = attr_docs.iter().any(|&doc| d.will_break(doc));

            let attr_concat = d.concat(&attr_docs);
            let attr_indent = d.indent(attr_concat);
            let inner = d.concat(&[
                d.text("<"),
                d.symbol(tag_sym),
                attr_indent,
                d.line(),
                d.text("/>"),
            ]);

            if has_multiline {
                d.group_break(inner)
            } else {
                d.group(inner)
            }
        }
    }

    /// Build opening tag with attributes
    ///
    /// When `force_break` is true (e.g., attribute value with embedded newlines),
    /// forces attributes to break across multiple lines.
    fn build_opening_tag(
        &self,
        tag_sym: u32,
        attr_docs: &[DocId],
        hug_start: bool,
        is_empty: bool,
        force_break: bool,
    ) -> DocId {
        let d = self.d();
        if attr_docs.is_empty() {
            d.concat(&[d.text("<"), d.symbol(tag_sym)])
        } else {
            let trailing = if hug_start && !is_empty {
                d.empty()
            } else {
                let sl = d.softline();
                d.dedent(sl)
            };
            let inner = d.concat(&[d.concat(attr_docs), trailing]);
            let attr_group = if force_break {
                d.group_break(inner)
            } else {
                d.group(inner)
            };
            let indented = d.indent(attr_group);
            d.concat(&[d.text("<"), d.symbol(tag_sym), indented])
        }
    }

    /// Build doc for element with content using boundary modes
    fn build_content_element_doc(
        &self,
        element: &internal::Element<'_>,
        ctx: &ElementContext,
        attr_docs: &[DocId],
        start_mode: BoundaryMode,
        end_mode: BoundaryMode,
        multiline_children: bool,
    ) -> DocId {
        let d = self.d();
        let tag_sym = element.name.to_u32();

        // Build children doc
        let children_doc = if multiline_children {
            // Multiline content: the prettier-shaped trimmed builder in `multiline` mode.
            let breakable_exprs = Self::nodes_have_breakable_expression(element.fragment.nodes);
            self.build_nodes_doc_trimmed(
                element.fragment.nodes,
                ctx.trim_boundaries,
                breakable_exprs,
                true,
            )
        } else if !(start_mode == BoundaryMode::Hug && end_mode == BoundaryMode::Hug) {
            self.build_nodes_doc_trimmed(element.fragment.nodes, ctx.trim_boundaries, false, false)
        } else {
            // Hug both: route through the prettier-shaped trimmed builder (the convergence
            // base). When the fragment carries a break-capable expression tag, opt into the
            // hard-width divergence so a long multi-expression run breaks the expression
            // rather than overshooting printWidth (`fill_multiple_expr_long`).
            let breakable_exprs = Self::nodes_have_breakable_expression(element.fragment.nodes);
            self.build_nodes_doc_trimmed(
                element.fragment.nodes,
                ctx.trim_boundaries,
                breakable_exprs,
                false,
            )
        };

        // Hug-both builds its own opening (the `>` is content-keyed, not attr-keyed), so handle it
        // before building `opening_tag` — every remaining arm uses `opening_tag`, this one doesn't.
        if start_mode == BoundaryMode::Hug && end_mode == BoundaryMode::Hug {
            return self.build_hug_both_doc(element, ctx, attr_docs, children_doc, false);
        }

        // Build opening tag
        let opening_tag = self.build_opening_tag(
            tag_sym,
            attr_docs,
            start_mode == BoundaryMode::Hug,
            ctx.is_empty,
            ctx.has_multiline_attr,
        );

        // Build doc structure based on boundary modes
        match (start_mode, end_mode) {
            (BoundaryMode::Hug, _) => {
                // Hug start: > hugs content
                let has_multiline_attrs = element.attributes.len() > 1;
                let leading_break = if ctx.source_has_leading_break || has_multiline_attrs {
                    d.hardline()
                } else {
                    d.softline()
                };
                let trailing_break = if end_mode == BoundaryMode::Hard {
                    d.hardline()
                } else {
                    d.softline()
                };
                let inner_group = d.group(d.concat(&[d.text(">"), children_doc]));
                let indent_inner = d.indent(d.concat(&[leading_break, inner_group]));
                d.group(d.concat(&[
                    opening_tag,
                    indent_inner,
                    trailing_break,
                    d.text("</"),
                    d.symbol(tag_sym),
                    d.text(">"),
                ]))
            }
            (_, BoundaryMode::Hug) => {
                // Hug end: content hugs closing tag
                let is_inline = ctx.kind.is_inline();
                let leading_break = if start_mode == BoundaryMode::Hard {
                    d.hardline()
                } else if is_inline
                    && Self::first_child_has_leading_ws(element.fragment.nodes, self.source)
                {
                    d.line()
                } else {
                    d.softline()
                };
                let trailing_break = if ctx.needs_multiline
                    || (ctx.kind.preserves_boundary_breaks() && ctx.source_has_trailing_break)
                {
                    d.hardline()
                } else {
                    d.softline()
                };
                // Rebuild children with trim=true for inline elements when
                // trim_boundaries was false, since line()/softline now provides
                // the boundary space that would otherwise duplicate.
                // Skip when multiline_children is true — the multiline doc already
                // handles whitespace correctly and must not be replaced with trimmed.
                let effective_children = if is_inline && !ctx.trim_boundaries && !multiline_children
                {
                    self.build_nodes_doc_trimmed(element.fragment.nodes, true, false, false)
                } else {
                    children_doc
                };
                let inner_group =
                    d.group(d.concat(&[effective_children, d.text("</"), d.symbol(tag_sym)]));
                let indent_inner = d.indent(d.concat(&[leading_break, inner_group]));
                d.group(d.concat(&[
                    opening_tag,
                    d.text(">"),
                    indent_inner,
                    trailing_break,
                    d.text(">"),
                ]))
            }
            (BoundaryMode::Hard, BoundaryMode::Hard) => {
                // Full multiline. Reuse the eagerly-built `children_doc` instead of
                // rebuilding the whole subtree: when `multiline_children`, it was built (above)
                // as `build_nodes_doc_trimmed(nodes, ctx.trim_boundaries, breakable, true)`, and
                // `build_nodes_doc_multiline` is the same call with `trim=true` hardcoded — so the
                // two are identical exactly when `ctx.trim_boundaries` is already true. Rebuilding
                // here is what made deeply-nested block content O(2^depth) (each level rebuilt its
                // children, which rebuilt theirs); the fallback keeps output byte-identical when the
                // eager doc used a different mode. See the build-fanout audit.
                let multiline_children_doc = if multiline_children && ctx.trim_boundaries {
                    children_doc
                } else {
                    self.build_nodes_doc_multiline(element.fragment.nodes)
                };
                let indent_inner = d.indent(d.concat(&[d.hardline(), multiline_children_doc]));
                d.concat(&[
                    opening_tag,
                    d.text(">"),
                    indent_inner,
                    d.hardline(),
                    d.text("</"),
                    d.symbol(tag_sym),
                    d.text(">"),
                ])
            }
            _ => {
                // Standard: soft breaks that can harden based on source
                //
                // For inline elements, use line() (space in flat, newline in break)
                // when the boundary text has whitespace. This matches Prettier's
                // printLineBeforeChildren (element.js:99-102) which returns `line`
                // when hasLeadingSpaces && isLeadingSpaceSensitive.
                //
                // line() handles both modes: space in flat, newline in break.
                // When trim_boundaries was false, rebuild children with trim=true
                // since line() now provides the boundary space.
                let is_inline = ctx.kind.is_inline();
                let leading_break = if start_mode == BoundaryMode::Hard {
                    d.hardline()
                } else if is_inline
                    && Self::first_child_has_leading_ws(element.fragment.nodes, self.source)
                {
                    d.line()
                } else {
                    d.softline()
                };
                let trailing_break = if end_mode == BoundaryMode::Hard {
                    d.hardline()
                } else if is_inline
                    && Self::last_child_has_trailing_ws(element.fragment.nodes, self.source)
                {
                    d.line()
                } else {
                    d.softline()
                };
                // Rebuild children with trim=true when trim_boundaries was false,
                // since line() now provides the boundary space that
                // handle_text_child would otherwise duplicate.
                let effective_children = if is_inline && !ctx.trim_boundaries {
                    self.build_nodes_doc_trimmed(element.fragment.nodes, true, false, false)
                } else {
                    children_doc
                };
                let inner_group = d.group(d.concat(&[effective_children]));
                let indent_inner = d.indent(d.concat(&[leading_break, inner_group]));
                d.group(d.concat(&[
                    opening_tag,
                    d.text(">"),
                    indent_inner,
                    trailing_break,
                    d.text("</"),
                    d.symbol(tag_sym),
                    d.text(">"),
                ]))
            }
        }
    }

    /// Build doc for hug-both mode (content hugs both opening and closing)
    ///
    /// When `external_close` is true the element's own trailing closing `>` (and the
    /// boundary break before it) is omitted — the caller emits the `>` elsewhere. This
    /// powers the axis-3 sibling-`>` dangle: an inline element directly followed by an
    /// expanding block renders as `</tag` and hands its `>` to the block so it can dangle
    /// onto the block-head line. See `build_inline_element_omit_close_gt`.
    fn build_hug_both_doc(
        &self,
        element: &internal::Element<'_>,
        ctx: &ElementContext,
        attr_docs: &[DocId],
        children_doc: DocId,
        external_close: bool,
    ) -> DocId {
        let tag_sym = element.name.to_u32();

        // `force` makes the content always-multiline (hardline boundaries) when an expanding
        // control-flow block (if/each/key, or nested in await), a non-inline snippet body, or
        // another multiline trigger is present; otherwise the softline boundaries
        // collapse-when-fits. (A control-flow-bearing *child* element already carries a hardline
        // that propagates `will_break`, so it needs no separate force term;
        // `source_has_leading_break` is impossible on the Hug/Hug path — there is no leading
        // boundary break — so it is dropped too.)
        let force = super::helpers::has_any_expanding_blocks(element.fragment.nodes)
            || ctx.block_flow_multiline
            || ctx.needs_multiline;

        // Opening is `<tag` (empty `attr_docs`) or the attr-keyed `build_opening_tag(hug_start=false)`,
        // whose `>` hugs the last attr when attrs fit and dedents to its own line when they wrap. The
        // attr group and the content group stay SEPARATE, so attr-wrapping and content-wrapping
        // decouple — the decoupling that makes the with-attrs case idempotent now that content no
        // longer flows on the tag lines. See conformance_prettier.md + the inline-layout lore.
        let opening =
            self.build_opening_tag(tag_sym, attr_docs, false, false, ctx.has_multiline_attr);

        self.build_inline_block_style(opening, tag_sym, children_doc, force, external_close)
    }

    /// Block-style inline content for [`Self::build_hug_both_doc`]. `opening` is everything before
    /// the content's `>` (`<tag` when there are no attrs, the attr-keyed `build_opening_tag(…)`
    /// otherwise); this appends `>`, puts the content on its own indented line(s) — collapsing to
    /// `<…>content</tag>` when it fits — and closes with `</tag>`. `force` ⇒ always multiline
    /// (hardline boundaries); otherwise softline collapses-when-fits. `external_close` drops the
    /// trailing `>` and its boundary break — the sibling-`>` dangle emits the `>` elsewhere.
    fn build_inline_block_style(
        &self,
        opening: DocId,
        tag_sym: u32,
        children_doc: DocId,
        force: bool,
        external_close: bool,
    ) -> DocId {
        let d = self.d();
        let leading = if force { d.hardline() } else { d.softline() };
        // External close: the trailing `>` and its preceding boundary break are emitted elsewhere,
        // so both collapse to nothing here.
        let trailing = if external_close {
            d.empty()
        } else if force {
            d.hardline()
        } else {
            d.softline()
        };
        let close_gt = if external_close {
            d.empty()
        } else {
            d.text(">")
        };
        let body = d.indent(d.concat(&[leading, children_doc]));
        d.group(d.concat(&[
            opening,
            d.text(">"),
            body,
            trailing,
            d.text("</"),
            d.symbol(tag_sym),
            close_gt,
        ]))
    }

    fn first_child_has_leading_ws(nodes: &[FragmentNode<'_>], source: &str) -> bool {
        nodes.first().is_some_and(
            |n| matches!(n, FragmentNode::Text(t) if !t.raw(source).leading_whitespace().is_empty()),
        )
    }

    fn last_child_has_trailing_ws(nodes: &[FragmentNode<'_>], source: &str) -> bool {
        nodes.last().is_some_and(
            |n| matches!(n, FragmentNode::Text(t) if !t.raw(source).trailing_whitespace().is_empty()),
        )
    }

    /// Build doc for empty element with no hugging
    ///
    /// For inline elements with whitespace-only content (e.g., `<span> </span>`),
    /// the space is preserved. When attrs force multiline, `>` and `</tag>` go
    /// on separate lines (matching Prettier behavior).
    fn build_empty_element_doc(
        &self,
        element: &internal::Element<'_>,
        opening_tag: DocId,
        has_attrs: bool,
        kind: ElementKind,
    ) -> DocId {
        let d = self.d();
        let tag_sym = element.name.to_u32();
        let is_inline = kind.is_inline();
        let is_html = element.kind == internal::ElementKind::Html;

        // Inline elements with whitespace-only content preserve a space
        // e.g., <span> </span> stays as-is, not collapsed to <span></span>
        // Matches prettier-plugin-svelte: isInlineElement = !isBlockElement
        let has_ws_content = is_inline
            && !element.fragment.nodes.is_empty()
            && element
                .fragment
                .nodes
                .iter()
                .all(FragmentNode::is_whitespace_only_text);

        if has_attrs && (is_inline || kind.is_component()) {
            // Closing for inline/hug states: "></tag>" or "> </tag>"
            let closing = if has_ws_content {
                d.concat(&[d.text("> </"), d.symbol(tag_sym), d.text(">")])
            } else {
                d.concat(&[d.text("></"), d.symbol(tag_sym), d.text(">")])
            };

            // Closing for full multiline state: with whitespace content,
            // > and </tag> go on separate lines; without, same as inline (hugged)
            let closing_multiline = if has_ws_content {
                let hl = d.hardline();
                d.concat(&[
                    d.text(">"),
                    hl,
                    d.text("</"),
                    d.symbol(tag_sym),
                    d.text(">"),
                ])
            } else {
                closing
            };

            // State 1: All inline
            let inline_state = d.concat(&[opening_tag, closing]);

            // State 2: Hug mode - attrs inline (space-separated), > on new line
            let hug_attrs = self.build_element_attrs_doc(
                element.attributes,
                self.d().text(" "),
                element.name_span.end,
                element.open_tag_end,
                is_html,
            );
            let hug_state = d.concat(&[
                d.text("<"),
                d.symbol(tag_sym),
                d.concat(&hug_attrs),
                d.hardline(),
                closing,
            ]);

            // State 3: Full multiline - attrs on separate lines, > on new line
            let multiline_attrs = self.build_element_attrs_doc(
                element.attributes,
                self.d().line(),
                element.name_span.end,
                element.open_tag_end,
                is_html,
            );
            let multiline_concat = d.concat(&multiline_attrs);
            let multiline_indent = d.indent(multiline_concat);
            let multiline_state = d.concat(&[
                d.text("<"),
                d.symbol(tag_sym),
                multiline_indent,
                d.hardline(),
                closing_multiline,
            ]);

            d.conditional_group(&[inline_state, hug_state, multiline_state])
        } else if has_ws_content {
            // Inline element with whitespace content, no attrs: <span> </span>
            d.concat(&[opening_tag, d.text("> </"), d.symbol(tag_sym), d.text(">")])
        } else {
            // Block elements or truly empty - use simple structure
            d.group(d.concat(&[opening_tag, d.text("></"), d.symbol(tag_sym), d.text(">")]))
        }
    }

    /// Build a doc for a `<template>` element with a foreign language (e.g., `lang="pug"`).
    /// Content is preserved raw — we can't format non-HTML template languages.
    /// Format: `<template lang="pug">\n{raw content}</template>`
    fn build_foreign_template_doc(&self, element: &internal::Element<'_>) -> DocId {
        let d = self.d();
        let tag_sym = element.name.to_u32();

        // Opening tag: <template attrs> — use space-separated attrs (no wrapping)
        // Foreign template elements are always HTML, so is_html=true
        let space_attrs = self.build_element_attrs_doc(
            element.attributes,
            self.d().text(" "),
            element.name_span.end,
            element.open_tag_end,
            true,
        );
        let mut parts: DocBuf = smallvec![d.text("<"), d.symbol(tag_sym)];
        parts.extend(space_attrs);
        parts.push(d.text(">"));

        // Raw content from fragment text nodes
        for node in element.fragment.nodes {
            if let FragmentNode::Text(text) = node {
                parts.push(d.text_owned(text.raw(self.source).to_string()));
            }
        }

        // Closing tag
        parts.push(d.text("</"));
        parts.push(d.symbol(tag_sym));
        parts.push(d.text(">"));

        d.concat(&parts)
    }

    /// Build a doc for a nested <style> or <script> element with formatted CSS/JS content
    ///
    /// This handles nested style/script elements (inside other elements like `<div>`)
    /// that need their content formatted as CSS/JS rather than as regular fragment nodes.
    pub(super) fn build_raw_content_element_doc(
        &self,
        tag_name: &str,
        element: &internal::Element<'_>,
        attr_docs: DocBuf,
    ) -> DocId {
        let d = self.d();
        let tag_sym = element.name.to_u32();
        // Build opening tag
        let opening_tag = if attr_docs.is_empty() {
            self.start_tag(tag_sym)
        } else {
            let sl = d.softline();
            let dedented = d.dedent(sl);
            let attr_concat = d.concat(&attr_docs);
            let inner = d.group(d.concat(&[attr_concat, dedented]));
            let indented = d.indent(inner);
            d.group(d.concat(&[d.text("<"), d.symbol(tag_sym), indented, d.text(">")]))
        };

        // Get raw content from the single Text child
        let content = element.fragment.nodes.first().and_then(|node| match node {
            FragmentNode::Text(text) => Some(text.data(self.source)),
            _ => None,
        });

        // Empty element or whitespace-only content
        let Some(content) = content.filter(|c| !c.trim().is_empty()) else {
            return d.concat(&[opening_tag, d.text("</"), d.symbol(tag_sym), d.text(">")]);
        };

        // Parse and format content based on tag type
        // Using base_indent_offset of 0 because we'll handle indentation in the doc structure.
        // The parse arena is a local: the parsed AST (CSS or TS) is consumed into an owned
        // formatted `String` here, so it never escapes this call. Pre-sized to the content
        // length to avoid the bump's chunk-doubling tail.
        let arena =
            bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(content.len()));
        let formatted = if tag_name == "style" {
            tsv_css::parse(&content, &arena)
                .ok()
                .map(|ast| tsv_css::format(&ast, &content))
        } else {
            tsv_ts::parse(&content, &arena)
                .ok()
                .map(|ast| tsv_ts::format(&ast, &content))
        };

        match formatted {
            Some(formatted) if !formatted.trim().is_empty() => {
                // Build doc with properly indented content
                // Each line of formatted content goes on its own line with indent
                let lines: Vec<&str> = formatted.trim_end().lines().collect();
                let mut content_lines: DocBuf = DocBuf::with_capacity(lines.len() * 2);
                for line in lines {
                    content_lines.push(d.hardline());
                    if !line.is_empty() {
                        content_lines.push(d.text_owned(line.to_string()));
                    }
                }

                let content_concat = d.concat(&content_lines);
                let indented = d.indent(content_concat);
                d.concat(&[
                    opening_tag,
                    indented,
                    d.hardline(),
                    d.text("</"),
                    d.symbol(tag_sym),
                    d.text(">"),
                ])
            }
            _ => {
                // Fallback: preserve raw content if parsing fails
                d.concat(&[
                    opening_tag,
                    d.text_owned(content.to_string()),
                    d.text("</"),
                    d.symbol(tag_sym),
                    d.text(">"),
                ])
            }
        }
    }

    /// Build docs for element attributes.
    ///
    /// `separator`: emitted between attributes — `d.line()` for the wrapping
    /// (line-separated) layout, `d.text(" ")` for hug mode (attributes stay on
    /// one line, space-separated).
    /// `name_end`: end position of the element tag name (for finding comments before first attr).
    /// `open_tag_end`: position of the `>` that closes the open tag (for trailing comment range).
    /// `is_html`: true for HTML elements, enables class attribute whitespace normalization.
    pub(crate) fn build_element_attrs_doc(
        &self,
        attrs: &[internal::AttributeNode<'_>],
        separator: DocId,
        name_end: u32,
        open_tag_end: u32,
        is_html: bool,
    ) -> DocBuf {
        // Most elements have a handful of attributes, so the per-element parts
        // buffer stays on the stack (`DocBuf`'s inline capacity); attribute-dense
        // elements spill to the heap as before.
        let mut docs: DocBuf = DocBuf::with_capacity(attrs.len() * 2);
        self.push_attrs_with_comments(&mut docs, attrs, separator, name_end, open_tag_end, is_html);
        docs
    }

    /// Push attribute docs with interleaved JS comment handling.
    ///
    /// Shared between regular element and special element attr doc builders.
    /// Handles comments between attributes (using `first_range_start` for the gap
    /// before the first attr) and trailing comments after the last attribute
    /// (bounded by `open_tag_end`).
    pub(super) fn push_attrs_with_comments(
        &self,
        docs: &mut DocBuf,
        attrs: &[internal::AttributeNode<'_>],
        separator: DocId,
        first_range_start: u32,
        open_tag_end: u32,
        is_html: bool,
    ) {
        let d = self.d();
        for (i, attr) in attrs.iter().enumerate() {
            // Check for JS comments before this attribute
            let range_start = if i == 0 {
                first_range_start
            } else {
                attrs[i - 1].span().end
            };
            let range_end = attr.span().start;

            if !tsv_lang::has_comments_in_range(self.comments, range_start, range_end) {
                docs.push(separator);
            } else {
                let comments: Vec<_> =
                    comments_in_range(self.comments, range_start, range_end).collect();
                let last_is_own_line = self.push_attr_comment_docs(docs, &comments, range_start);
                // Separator before the next attribute
                if last_is_own_line {
                    docs.push(d.hardline());
                } else {
                    docs.push(d.text(" "));
                }
            }

            docs.push(self.build_attribute_node_doc(attr, is_html));
        }

        // Check for trailing comments after last attribute
        if let Some(last_attr) = attrs.last() {
            let range_start = last_attr.span().end;
            if tsv_lang::has_comments_in_range(self.comments, range_start, open_tag_end) {
                let trailing: Vec<_> =
                    comments_in_range(self.comments, range_start, open_tag_end).collect();
                self.push_attr_comment_docs(docs, &trailing, range_start);
            }
        }
    }

    /// Push docs for JS comments between attributes.
    ///
    /// Each comment gets a preceding separator (hardline when it starts its own
    /// line, an inline space when it trails the previous token). Returns whether
    /// the following attribute must start on a new line — true for any own-line
    /// comment and for any line comment (a `//` runs to end of line, so the next
    /// token can't share it); the caller uses this to pick that separator.
    pub(super) fn push_attr_comment_docs(
        &self,
        docs: &mut DocBuf,
        comments: &[&tsv_lang::Comment],
        range_start: u32,
    ) -> bool {
        let d = self.d();
        let mut last_was_own_line = false;
        for comment in comments {
            let is_own_line =
                self.source[range_start as usize..comment.span.start as usize].contains('\n');

            // Preserve the author's placement: a comment on its own line stays on its
            // own line; a comment on the same line as the preceding token stays
            // trailing it (inline). This already held for block comments; it now
            // extends to line comments (a `//` the author put after the tag name or
            // an attribute is kept there rather than relocated to its own line).
            if is_own_line {
                docs.push(d.hardline());
            } else {
                docs.push(d.text(" "));
            }
            docs.push(self.build_attr_js_comment_doc(comment));
            if !comment.is_block {
                // A `//` runs to end of line, so the following attribute or the
                // closing `>` / `/>` must drop to the next line — force the open-tag
                // group to break so it can't be swallowed into the comment.
                docs.push(d.break_parent());
            }
            // A line comment always pushes the next token to a new line; a same-line
            // block comment lets it stay inline.
            last_was_own_line = is_own_line || !comment.is_block;
        }
        last_was_own_line
    }

    /// Build a doc for a JS comment's text (without surrounding separators)
    pub(super) fn build_attr_js_comment_doc(&self, comment: &tsv_lang::Comment) -> DocId {
        let d = self.d();
        if comment.is_block {
            d.concat(&[
                d.text("/*"),
                d.text_owned(comment.content(self.source).to_string()),
                d.text("*/"),
            ])
        } else {
            d.concat(&[
                d.text("//"),
                d.text_owned(comment.content(self.source).to_string()),
            ])
        }
    }

    /// Whether the source slice for `span` ends with a self-closing `/>` (for doc
    /// building). Shared by regular and special elements.
    pub(super) fn span_was_self_closing(&self, span: Span) -> bool {
        span.extract(self.source).trim_end().ends_with("/>")
    }
}
