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
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::{DocBuf, arena::DocId};

/// How content relates to an element boundary (opening or closing tag)
///
/// This determines what separator (if any) appears between the tag and content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundaryMode {
    /// Hardline separator - preserves source structure
    /// Example: `<p>\n  text` (source had newline, preserve it)
    Hard,
    /// Softline separator - collapses or breaks based on fit
    /// Example: `<span> text` and `<span>text` alike: the authored boundary run is
    /// render-free, so both collapse onto the tag line when the content fits and break
    /// block-style when it doesn't.
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
    /// Element with content. ONE boundary mode covers both tags: they always move together, so
    /// that a render-free boundary character can never dangle one delimiter without the other
    /// (see [`Printer::compute_element_layout`]). `Hard` is exactly the multiline case — the
    /// children are built one-per-line iff the boundaries are hard.
    WithContent(BoundaryMode),
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

/// The element-shaped inputs the shared analyze → layout → build pipeline reads.
///
/// [`internal::Element`] and [`internal::SpecialElement`] are distinct AST types that print
/// the same shape: a name, attributes, a fragment, and an open/close tag pair. Projecting
/// both onto one view lets the layout decisions (multiline-ness, boundary modes, hugging)
/// live in a single place. They used to be duplicated — `special_doc.rs` carried its own
/// hug predicates and its own `needs_multiline`, and the copies had drifted: `<slot>` never
/// went multiline for block children, and the special path still dangled its delimiters
/// where regular elements had moved to block-style.
///
/// `name` is the tag-name doc, reused by both the opening and the closing tag (a span-identity
/// `source_span` slice for a regular element, static text for a `svelte:*` one).
#[derive(Clone, Copy)]
pub(super) struct ElementParts<'arena> {
    pub(super) name: DocId,
    pub(super) kind: ElementKind,
    /// Void element (`<br>`, `<img>`) — no closing tag
    pub(super) is_void: bool,
    /// Whether an empty element may print self-closing when the source wrote it that way
    pub(super) can_self_close: bool,
    /// A whitespace-collapsing container (`<table>`, `<select>`, …): the compiler removes
    /// inter-sibling whitespace entirely, so the content lays out block-style with it trimmed.
    pub(super) collapses_child_ws: bool,
    pub(super) nodes: &'arena [FragmentNode<'arena>],
    pub(super) span: Span,
}

/// Everything the printer derives from an element's tag NAME.
///
/// Unpacked from the parse-time `Element::facts` ([`TagFacts`](internal::TagFacts)) by
/// `classify_tag`, so the printer re-derives nothing per element — one field read, no
/// per-element `String`. Emission is a span-identity `source_span` slice of the tag name.
///
/// A named struct rather than a tuple: these are seven independent bools that would otherwise
/// be positional and silently misorderable at the call site (the same reason
/// [`MultilineInputs`](super::element_analysis) exists).
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy)]
pub(super) struct TagClass {
    pub(super) kind: ElementKind,
    /// `<br>`, `<img>` — no closing tag
    pub(super) is_void: bool,
    /// SVG / MathML — may print self-closing like a component
    pub(super) is_foreign: bool,
    /// `<foo:bar>` — a namespaced regular element; inline-kinded, but may print self-closing
    pub(super) is_namespaced: bool,
    pub(super) is_style: bool,
    pub(super) is_script: bool,
    pub(super) is_template: bool,
    /// `<pre>` / `<textarea>` — content whitespace is literal
    pub(super) is_ws_sensitive: bool,
    /// `<table>` / `<select>` / … — a whitespace-collapsing container: the compiler removes
    /// inter-sibling whitespace entirely (`clean_nodes` `can_remove_entirely`), so tsv lays the
    /// content out block-style with the inter-sibling whitespace trimmed.
    pub(super) collapses_child_ws: bool,
    /// `<!DOCTYPE>` — closes with `>`, not `/>`
    pub(super) is_declaration: bool,
}

/// The source window an attribute list's comment gaps live in.
///
/// A named struct rather than loose `u32`s for the same reason [`TagClass`] is one: the two
/// offsets are positional and silently swappable at the call site.
#[derive(Clone, Copy)]
pub(super) struct AttrGaps {
    /// Where the gap before the first attribute starts — the tag name's end.
    pub(super) first_range_start: u32,
    /// The `>` closing the opening tag; bounds the gap after the last attribute.
    pub(super) open_tag_end: u32,
    /// A region inside the window that the caller prints itself, whose comments the scan
    /// must therefore skip. `<svelte:element this={…}>` keeps its `this` out of the
    /// attribute list and synthesizes the attribute, so the braces land in one of the gaps
    /// probed here while the tag's own doc is what prints them; without the skip a comment
    /// in there is emitted twice, once by each. Ownership does not cover this on its own: a
    /// *glued block* comment is `owned_by_node` and already skipped on the `to emit` axis,
    /// but a line comment never is (`owned ⇒ is_block`).
    pub(super) claimed: Option<Span>,
}

/// Analysis context for element formatting decisions
///
/// Computed once per element from its [`ElementParts`], used to determine layout and build
/// docs. Strictly the *derived* half — anything readable straight off `ElementParts` (the tag
/// kind, void-ness) stays there, so no fact has two sources that could drift apart.
#[allow(clippy::struct_excessive_bools)]
pub(super) struct ElementContext {
    /// Whether element was self-closing in source
    pub(super) is_self_closing: bool,
    /// Whether element has no meaningful content
    pub(super) is_empty: bool,
    /// Whether children need multiline formatting
    pub(super) needs_multiline: bool,
    /// Whether any attribute source contains embedded newlines (forces attr group break)
    pub(super) has_multiline_attr: bool,
}

impl<'a> Printer<'a> {
    /// `<name>` — a start tag with no attributes (HTML spec "start tag").
    /// `name` is a pre-built name doc (span-identity `source_span`).
    #[inline]
    pub(super) fn start_tag(&self, name: DocId) -> DocId {
        let d = self.d();
        d.concat(&[d.text("<"), name, d.text(">")])
    }

    /// `</name>` — an end tag (HTML spec "end tag").
    #[inline]
    pub(super) fn end_tag(&self, name: DocId) -> DocId {
        let d = self.d();
        d.concat(&[d.text("</"), name, d.text(">")])
    }

    /// Unpack an element's parse-time name facts (`Element::facts`) into the printer's per-tag
    /// view. The single classifier — both element entry points go through it, so they cannot drift.
    pub(super) fn classify_tag(&self, element: &internal::Element<'_>) -> TagClass {
        let facts = element.facts;
        // Element kind, matching prettier-plugin-svelte's isInlineElement = !isBlockElement:
        // elements NOT in the block list (table cells included) use inline formatting.
        let kind = if facts.is_component_name() {
            ElementKind::Component
        } else if facts.is_block() {
            ElementKind::Block
        } else {
            ElementKind::Inline
        };
        TagClass {
            kind,
            is_void: facts.is_void(),
            is_foreign: facts.is_foreign(),
            is_namespaced: facts.is_namespaced(),
            is_style: facts.is_style(),
            is_script: facts.is_script(),
            is_template: facts.is_template(),
            is_ws_sensitive: facts.is_ws_sensitive(),
            collapses_child_ws: facts.collapses_child_whitespace(),
            is_declaration: facts.is_declaration(),
        }
    }

    /// Project a regular element onto the shared [`ElementParts`] view.
    pub(super) fn element_parts<'e>(
        &self,
        element: &'e internal::Element<'e>,
        class: TagClass,
    ) -> ElementParts<'e> {
        ElementParts {
            name: self.d().source_span_ident(element.name_span),
            kind: class.kind,
            is_void: class.is_void,
            // Components, foreign (SVG/MathML), and namespaced (`foo:bar`) elements may print
            // self-closing (prettier's `didSelfClose`).
            can_self_close: class.kind.is_component() || class.is_foreign || class.is_namespaced,
            collapses_child_ws: class.collapses_child_ws,
            nodes: element.fragment.nodes,
            span: element.span,
        }
    }

    /// Build a doc for an element (regular HTML or component)
    ///
    /// Uses a three-phase approach:
    /// 1. Analyze: Compute all formatting-relevant properties
    /// 2. Classify: Determine layout strategy (void, empty, hug modes, etc.)
    /// 3. Build: Construct doc based on layout
    pub(crate) fn build_element_doc(&self, element: &internal::Element<'_>) -> DocId {
        let class = self.classify_tag(element);
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
        if class.is_style || class.is_script {
            return self.build_raw_content_element_doc(class.is_style, element, attr_docs);
        }

        // Foreign language <template> elements (e.g., <template lang="pug">)
        // preserve content raw — we can't format non-HTML template languages
        if class.is_template
            && let Some(lang) = self.get_lang_attribute(element.attributes)
            && lang != "html"
        {
            return self.build_foreign_template_doc(element);
        }

        // Whitespace-sensitive elements (pre, textarea, etc.) — these keep the mandatory
        // delimiter dangle, so they must never reach the shared layout analysis below.
        if class.is_ws_sensitive {
            return self.build_whitespace_sensitive_element_doc(element, attr_docs);
        }

        let parts = self.element_parts(element, class);

        // Phase 1: Analyze element
        let ctx = self.analyze_element(&parts, &attr_docs);

        // Phase 2: Compute layout
        let layout = self.compute_element_layout(&parts, &ctx);

        // Phase 3: Build doc based on layout
        match layout {
            ElementLayout::Void | ElementLayout::SelfClosing => {
                // DOCTYPE uses > (no self-closing slash) — it's a declaration, not an element
                self.build_void_element_doc(&parts, &attr_docs, class.is_declaration)
            }
            ElementLayout::Empty => {
                let opening_tag =
                    self.build_opening_tag(parts.name, &attr_docs, ctx.has_multiline_attr);
                self.build_empty_element_doc(
                    element,
                    opening_tag,
                    !attr_docs.is_empty(),
                    class.kind,
                )
            }
            ElementLayout::WithContent(boundary) => {
                self.build_content_element_doc(&parts, &ctx, &attr_docs, boundary)
            }
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
        self.build_inline_element_sibling_gt(element, true, None)
    }

    /// Shared body for the axis-3 element sibling-`>` roles, composable so one element can play
    /// **both** at once inside a glued run (`build_glued_element_run`): it **sheds** its closing
    /// `>` to the following sibling (`external_close = true`) and/or **receives** a preceding
    /// sibling's `>` as a leading `if_break` inside its attrs group (`gt_prefix = Some`) — a mid-run
    /// element does both. The single render-safe shape is a flat hug-both (`Soft`) content layout,
    /// so a wrong element kind returns `None` and the caller keeps that boundary an intact `>`.
    pub(crate) fn build_inline_element_sibling_gt(
        &self,
        element: &internal::Element<'_>,
        external_close: bool,
        gt_prefix: Option<DocId>,
    ) -> Option<DocId> {
        // Special-content elements (raw `<script>`/`<style>`, foreign `<template>`,
        // whitespace-sensitive `<pre>`/`<textarea>`) never participate — their closing
        // tags aren't the simple hug-both shape.
        let class = self.classify_tag(element);
        if class.is_style
            || class.is_script
            || class.is_ws_sensitive
            || (class.is_template
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
        let parts = self.element_parts(element, class);
        let ctx = self.analyze_element(&parts, &attr_docs);
        // Only a flat content layout has a single trailing `>` we can cleanly split off:
        // Hug (glued boundaries), and Soft (a collapsing boundary — a one-sided newline or
        // a render-free run — that trims to the same glued form; without the dangle here,
        // format(one-sided-newline authoring) would emit the glued no-dangle form, which
        // the next pass reads as Hug and dangles — a non-idempotent 2-cycle,
        // `authoring_audit`'s hard bucket). Multiline children (Hard) and the
        // void/empty/self-closing forms keep their `>` (return None → no dangle).
        match self.compute_element_layout(&parts, &ctx) {
            ElementLayout::WithContent(BoundaryMode::Soft) => {
                // Children built exactly as `build_content_element_doc`'s Hug arm builds
                // them (trimmed), so the dangled element renders its content identically
                // to its undangled form — incl. trimming a render-free boundary space
                // (`<span>text </span>{#each…}` must dangle like the glued authoring).
                let children_doc = self.build_nodes_doc_trimmed(
                    element.fragment.nodes,
                    Self::nodes_have_breakable_expression(element.fragment.nodes),
                    false,
                );
                Some(self.build_collapsible_element_doc(
                    &parts,
                    &ctx,
                    &attr_docs,
                    children_doc,
                    external_close,
                    gt_prefix,
                ))
            }
            _ => None,
        }
    }

    /// Build doc for void or self-closing element
    ///
    /// When any attribute doc will_break (e.g., multiline string value),
    /// forces attributes to break across multiple lines to match Prettier behavior.
    pub(super) fn build_void_element_doc(
        &self,
        parts: &ElementParts<'_>,
        attr_docs: &[DocId],
        is_declaration: bool,
    ) -> DocId {
        let d = self.d();
        let name = parts.name;
        // Declarations (<!DOCTYPE>) use > without self-closing slash
        if attr_docs.is_empty() {
            if is_declaration {
                d.concat(&[d.text("<"), name, d.text(">")])
            } else {
                d.concat(&[d.text("<"), name, d.text(" />")])
            }
        } else if is_declaration {
            let attr_concat = d.concat(attr_docs);
            let attr_indent = d.indent(attr_concat);
            let inner = d.concat(&[d.text("<"), name, attr_indent, d.softline(), d.text(">")]);
            d.group(inner)
        } else {
            // Check if any attribute doc will break (contains hardline)
            let has_multiline = attr_docs.iter().any(|&doc| d.will_break(doc));

            let attr_concat = d.concat(attr_docs);
            let attr_indent = d.indent(attr_concat);
            let inner = d.concat(&[d.text("<"), name, attr_indent, d.line(), d.text("/>")]);

            if has_multiline {
                d.group_break(inner)
            } else {
                d.group(inner)
            }
        }
    }

    /// Build an opening tag up to (but not including) its closing `>` — the caller emits that,
    /// since where it lands is the caller's layout decision.
    ///
    /// The `>` is **attr-keyed**: the trailing dedented softline hugs it to the last attribute
    /// when the attributes fit and drops it to its own line when they wrap. When `force_break`
    /// is true (e.g. an attribute value with embedded newlines) the attributes always wrap.
    pub(super) fn build_opening_tag(
        &self,
        name: DocId,
        attr_docs: &[DocId],
        force_break: bool,
    ) -> DocId {
        let d = self.d();
        if attr_docs.is_empty() {
            d.concat(&[d.text("<"), name])
        } else {
            // Always the attr-keyed trailing break. main's `hug_start && !is_empty` fast path
            // (emit the attr concat alone, skipping an `empty()` child) optimized a branch this
            // file no longer has: a hugged open tag used to suppress the trailing break, which is
            // exactly the delimiter-dangle machinery the block-style stance removed. There is no
            // `empty()` child left to avoid, and `hug_start`/`is_empty` are no longer parameters.
            let sl = d.softline();
            let inner = d.concat(&[d.concat(attr_docs), d.dedent(sl)]);
            let attr_group = if force_break {
                d.group_break(inner)
            } else {
                d.group(inner)
            };
            let indented = d.indent(attr_group);
            d.concat(&[d.text("<"), name, indented])
        }
    }

    /// Build an opening tag whose leading `>` (`gt`) belongs to a **preceding glued inline
    /// element** whose closing tag shed it (the axis-3 sibling-`>` dangle extended to an
    /// element→element chain, "G2"). The `gt` sits as a leading `if_break([hardline, gt], gt)`
    /// **inside** this tag's own attrs group, so it reads that group's break decision: when the
    /// attributes wrap (`</span⏎><a⏎…`) the `>` drops with a hardline onto this tag's line; when
    /// they fit flat (`</span><a…`) the `>` hugs. Placing the `<name` inside the group too (unlike
    /// [`Self::build_opening_tag`], where it sits outside) is what lets the id-less `if_break`
    /// read the attrs group — an `if_break` binds to its nearest enclosing `Group`.
    fn build_opening_tag_with_gt_prefix(
        &self,
        name: DocId,
        attr_docs: &[DocId],
        force_break: bool,
        gt: DocId,
    ) -> DocId {
        let d = self.d();
        if attr_docs.is_empty() {
            // No attrs ⇒ this tag can never wrap ⇒ the `>` always hugs, statically.
            return d.concat(&[gt, d.text("<"), name]);
        }
        let sl = d.softline();
        let attrs_body = d.indent(d.concat(&[d.concat(attr_docs), d.dedent(sl)]));
        let prefix = d.if_break(d.concat(&[d.hardline(), gt]), gt);
        let whole = d.concat(&[prefix, d.text("<"), name, attrs_body]);
        if force_break {
            d.group_break(whole)
        } else {
            d.group(whole)
        }
    }

    /// Build doc for element with content using boundary modes.
    ///
    /// Every arm here is **block-style**: both tags stay intact and the content moves to its own
    /// indented line(s) when it breaks. A delimiter never dangles — the only boundary modes that
    /// reach this point are Hug/Hug (all-or-nothing, see [`Printer::compute_element_layout`]),
    /// Hard, and Soft, and a Soft boundary in break mode is a plain newline before the closing
    /// tag. (`<pre>`/`<textarea>`, where the dangle IS mandatory, never reach this builder.)
    pub(super) fn build_content_element_doc(
        &self,
        parts: &ElementParts<'_>,
        ctx: &ElementContext,
        attr_docs: &[DocId],
        boundary: BoundaryMode,
    ) -> DocId {
        let d = self.d();
        let nodes = parts.nodes;

        // Build the children doc EXACTLY ONCE, in the variant the resolved boundary arm
        // actually uses (rebuilding per arm recursed into children that ALSO rebuilt, making
        // deeply nested inline content O(2^depth) — see the build-fanout audit). Boundary
        // whitespace is always trimmed: it is render-free under Svelte 5 (`clean_nodes` trims
        // every fragment edge at compile), so no element kind keeps it. Only the multiline-ness
        // varies — `Hard` is exactly the multiline case.
        //
        // `breakable_exprs` (the fill-vs-hard-width divergence for long multi-expression runs,
        // `fill_multiple_expr_long`) is a property of the CONTENT (does a child `{expr}` have
        // internal break points?), not of the boundary, so it cannot depend on which boundary
        // mode we landed in. Passing `false` strands a breakable expression group flat under a
        // fits()-Break `line` — which, on a fill whose text and ternaries compete for the same
        // line, oscillates between two layouts (a non-idempotent 2-cycle, `authoring_audit`'s
        // hard bucket).
        // A whitespace-collapsing container lays its children out one-per-line with the
        // inter-sibling whitespace trimmed (render-free — the compiler removes it). Its
        // `needs_multiline` is forced (see `analyze_element`), so `boundary` is always `Hard` here
        // and this content flows into the multiline arm below.
        let children_doc = if parts.collapses_child_ws {
            self.build_container_content_doc(nodes)
        } else {
            self.build_nodes_doc_trimmed(
                nodes,
                Self::nodes_have_breakable_expression(nodes),
                boundary == BoundaryMode::Hard,
            )
        };

        // Soft boundaries: collapse when the element fits, break block-style when it doesn't.
        //
        // Always softlines: an authored boundary space is render-free (the compiler trims every
        // fragment edge), so it neither survives inline — `<span> text </span>` collapses to
        // `<span>text</span>` — nor selects the layout. Prettier instead keeps the space
        // (`printLineBeforeChildren`'s `line` when hasLeadingSpaces && isLeadingSpaceSensitive,
        // the HTML/CSS inline whitespace model Svelte 5 broke from) — see
        // conformance_prettier.md §Svelte: Inline content block-style and the
        // inline_boundary_whitespace fixture.
        if boundary == BoundaryMode::Soft {
            return self.build_collapsible_element_doc(
                parts,
                ctx,
                attr_docs,
                children_doc,
                false,
                None,
            );
        }

        // Full multiline. `children_doc` was built once above as the multiline shape
        // (`build_nodes_doc_multiline` == `build_nodes_doc_trimmed(nodes, true, breakable,
        // true)`); rebuilding here per level is what made deeply-nested content O(2^depth).
        let opening_tag = self.build_opening_tag(parts.name, attr_docs, ctx.has_multiline_attr);
        let indent_inner = d.indent(d.concat(&[d.hardline(), children_doc]));
        d.concat(&[
            opening_tag,
            d.text(">"),
            indent_inner,
            d.hardline(),
            d.text("</"),
            parts.name,
            d.text(">"),
        ])
    }

    /// Build doc for the collapsible (`Soft`) content layout — the single inline shape, whatever
    /// the author wrote at the boundary.
    ///
    /// Softline boundaries: the content collapses onto the tag line when it fits and drops to its
    /// own indented line (block-style, both tags intact) when it doesn't. Since the boundary run
    /// is render-free and always trimmed, a glued authoring and a spaced one reach this same
    /// builder — that is what makes them converge. No hardline force is needed — every multiline
    /// trigger (an expanding control-flow block, block-flow children, any other `needs_multiline`)
    /// already resolves the boundary to `Hard` in [`Printer::compute_element_layout`], so it never
    /// reaches this builder.
    ///
    /// When `external_close` is true the element's own trailing closing `>` (and the boundary
    /// break before it) is omitted — the caller emits the `>` elsewhere. This powers the axis-3
    /// sibling-`>` dangle: an inline element directly followed by an expanding block renders as
    /// `</tag` and hands its `>` to the block so it can dangle onto the block-head line. See
    /// [`Printer::build_inline_element_omit_close_gt`].
    fn build_collapsible_element_doc(
        &self,
        parts: &ElementParts<'_>,
        ctx: &ElementContext,
        attr_docs: &[DocId],
        children_doc: DocId,
        external_close: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();

        // Opening is `<tag` (empty `attr_docs`) or the attr-keyed `build_opening_tag`, whose `>`
        // hugs the last attr when attrs fit and dedents to its own line when they wrap. The attr
        // group and the content group stay SEPARATE, so attr-wrapping and content-wrapping
        // decouple — the decoupling that makes the with-attrs case idempotent now that content no
        // longer flows on the tag lines. See conformance_prettier.md.
        //
        // `gt_prefix` (Some) is a preceding glued element's shed `>`, threaded into this tag's
        // attrs group as a leading `if_break` (the G2 sibling-`>` dangle) — see
        // [`Self::build_opening_tag_with_gt_prefix`].
        let opening = match gt_prefix {
            Some(gt) => self.build_opening_tag_with_gt_prefix(
                parts.name,
                attr_docs,
                ctx.has_multiline_attr,
                gt,
            ),
            None => self.build_opening_tag(parts.name, attr_docs, ctx.has_multiline_attr),
        };

        // External close: the trailing `>` and its preceding boundary break are emitted elsewhere,
        // so both collapse to nothing here.
        let (trailing, close_gt) = if external_close {
            (d.empty(), d.empty())
        } else {
            (d.softline(), d.text(">"))
        };
        let body = d.indent(d.concat(&[d.softline(), children_doc]));
        d.group(d.concat(&[
            opening,
            d.text(">"),
            body,
            trailing,
            d.text("</"),
            parts.name,
            close_gt,
        ]))
    }

    /// Build doc for empty element with no hugging
    ///
    /// A whitespace-only fragment counts as empty for every element kind — `<b> </b>`
    /// collapses to `<b></b>` (Svelte renders nothing there: the boundary run is trimmed at
    /// compile, so the space is render-free; prettier preserves it — see
    /// conformance_prettier.md §Svelte: Inline content block-style). When attrs force
    /// multiline, `>` and `</tag>` go on separate lines (matching Prettier behavior).
    fn build_empty_element_doc(
        &self,
        element: &internal::Element<'_>,
        opening_tag: DocId,
        has_attrs: bool,
        kind: ElementKind,
    ) -> DocId {
        let d = self.d();
        let name_doc = d.source_span_ident(element.name_span);
        let is_html = element.kind == internal::ElementKind::Html;
        let closing = d.concat(&[d.text("></"), name_doc, d.text(">")]);

        if has_attrs && (kind.is_inline() || kind.is_component()) {
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
                name_doc,
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
                name_doc,
                multiline_indent,
                d.hardline(),
                closing,
            ]);

            d.conditional_group(&[inline_state, hug_state, multiline_state])
        } else {
            d.group(d.concat(&[opening_tag, closing]))
        }
    }

    /// Build a doc for a `<template>` element with a foreign language (e.g., `lang="pug"`).
    /// Content is preserved raw — we can't format non-HTML template languages.
    /// Format: `<template lang="pug">\n{raw content}</template>`
    fn build_foreign_template_doc(&self, element: &internal::Element<'_>) -> DocId {
        let d = self.d();
        let name_doc = d.source_span_ident(element.name_span);

        // Opening tag: <template attrs> — use space-separated attrs (no wrapping)
        // Foreign template elements are always HTML, so is_html=true
        let space_attrs = self.build_element_attrs_doc(
            element.attributes,
            self.d().text(" "),
            element.name_span.end,
            element.open_tag_end,
            true,
        );
        let mut parts: DocBuf = smallvec![d.text("<"), name_doc];
        parts.extend(space_attrs);
        parts.push(d.text(">"));

        // Raw content from fragment text nodes
        for node in element.fragment.nodes {
            if let FragmentNode::Text(text) = node {
                parts.push(d.source_span(text.raw_span, self.source));
            }
        }

        // Closing tag
        parts.push(d.text("</"));
        parts.push(name_doc);
        parts.push(d.text(">"));

        d.concat(&parts)
    }

    /// Build a doc for a nested <style> or <script> element with formatted CSS/JS content
    ///
    /// This handles nested style/script elements (inside other elements like `<div>`)
    /// that need their content formatted as CSS/JS rather than as regular fragment nodes.
    pub(super) fn build_raw_content_element_doc(
        &self,
        is_style: bool,
        element: &internal::Element<'_>,
        attr_docs: DocBuf,
    ) -> DocId {
        let d = self.d();
        let name_doc = d.source_span_ident(element.name_span);
        // Build opening tag
        let opening_tag = if attr_docs.is_empty() {
            self.start_tag(name_doc)
        } else {
            let sl = d.softline();
            let dedented = d.dedent(sl);
            let attr_concat = d.concat(&attr_docs);
            let inner = d.group(d.concat(&[attr_concat, dedented]));
            let indented = d.indent(inner);
            d.group(d.concat(&[d.text("<"), name_doc, indented, d.text(">")]))
        };

        // Get raw content from the single Text child
        let content = element.fragment.nodes.first().and_then(|node| match node {
            FragmentNode::Text(text) => Some(text.data(self.source)),
            _ => None,
        });

        // Empty element or whitespace-only content
        let Some(content) = content.filter(|c| !c.trim().is_empty()) else {
            return d.concat(&[opening_tag, d.text("</"), name_doc, d.text(">")]);
        };

        // Parse and format content based on tag type
        // Using base_indent_offset of 0 because we'll handle indentation in the doc structure.
        // The parse arena is a local: the parsed AST (CSS or TS) is consumed into an owned
        // formatted `String` here, so it never escapes this call. Pre-sized to the content
        // length to avoid the bump's chunk-doubling tail.
        let arena =
            bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(content.len()));
        // Format into the host document's doc arena rather than a fresh per-element
        // one — the same arena-sharing as the top-level `<style>`/`<script>` path
        // (`format_embedded_in` / the TS build helpers). `format_in` is
        // output-identical to `format`; the parsed content renders to an owned
        // `String` here, so nothing borrowed from the arena escapes and the arena
        // is not reset.
        let formatted = if is_style {
            tsv_css::parse(&content, &arena)
                .ok()
                .map(|ast| tsv_css::format_in(&ast, &content, self.d()))
        } else {
            let parsed = tsv_ts::parse(&content, &arena).ok();
            parsed.map(|ast| tsv_ts::format_in(&ast, &content, self.d()))
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
                        content_lines.push(d.text_pooled(line));
                    }
                }

                let content_concat = d.concat(&content_lines);
                let indented = d.indent(content_concat);
                d.concat(&[
                    opening_tag,
                    indented,
                    d.hardline(),
                    d.text("</"),
                    name_doc,
                    d.text(">"),
                ])
            }
            _ => {
                // Fallback: preserve raw content if parsing fails
                d.concat(&[
                    opening_tag,
                    d.text_pooled(&content),
                    d.text("</"),
                    name_doc,
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
        self.push_attrs_with_comments(
            &mut docs,
            attrs,
            separator,
            AttrGaps {
                first_range_start: name_end,
                open_tag_end,
                // A regular element's attributes are all in `attrs` — nothing here is
                // printed by a synthesized attribute of the caller's own.
                claimed: None,
            },
            is_html,
        );
        docs
    }

    /// Push attribute docs with interleaved JS comment handling.
    ///
    /// Shared between regular element and special element attr doc builders.
    /// Handles comments between attributes and trailing comments after the last one, over
    /// the window described by [`AttrGaps`].
    pub(super) fn push_attrs_with_comments(
        &self,
        docs: &mut DocBuf,
        attrs: &[internal::AttributeNode<'_>],
        separator: DocId,
        gaps: AttrGaps,
        is_html: bool,
    ) {
        let d = self.d();
        let AttrGaps {
            first_range_start,
            open_tag_end,
            claimed,
        } = gaps;
        // The gap probes below all go through these, so the claimed region is skipped once
        // here rather than at each of the four sites.
        let gap_comments = |start: u32, end: u32| {
            comments_to_emit_in_range(self.comments, start, end).filter(move |c| {
                !claimed.is_some_and(|r| r.start <= c.span.start && c.span.end <= r.end)
            })
        };
        let has_gap_comments = |start: u32, end: u32| gap_comments(start, end).next().is_some();

        // Every gap this fn probes — each attribute's leading range and the trailing range
        // after the last one — lies inside `[first_range_start, open_tag_end]`. A comment
        // lands in a probe only when it sits fully inside the queried range, so a
        // comment-free open tag means every one of those gaps is comment-free: each would
        // take the bare-separator branch below and the trailing block would emit nothing.
        // Answer that with one probe instead of one per attribute plus one. (Whole-window
        // and per-gap probes share `gap_comments`, so the claim is honored identically by
        // both — a claim that shortcut only the per-gap probes would re-open the
        // double-print through this fast path.)
        if !has_gap_comments(first_range_start, open_tag_end) {
            for attr in attrs {
                docs.push(separator);
                docs.push(self.build_attribute_node_doc(attr, is_html));
            }
            return;
        }

        for (i, attr) in attrs.iter().enumerate() {
            // Check for JS comments before this attribute
            let range_start = if i == 0 {
                first_range_start
            } else {
                attrs[i - 1].span().end
            };
            let range_end = attr.span().start;

            if !has_gap_comments(range_start, range_end) {
                docs.push(separator);
            } else {
                let last_is_own_line = self.push_attr_comment_docs(
                    docs,
                    gap_comments(range_start, range_end),
                    range_start,
                );
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
            if has_gap_comments(range_start, open_tag_end) {
                self.push_attr_comment_docs(
                    docs,
                    gap_comments(range_start, open_tag_end),
                    range_start,
                );
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
    pub(super) fn push_attr_comment_docs<'c>(
        &self,
        docs: &mut DocBuf,
        comments: impl IntoIterator<Item = &'c tsv_lang::Comment>,
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
        let doc = if comment.is_block {
            d.concat(&[
                d.text("/*"),
                d.source_span(comment.content_span, self.source),
                d.text("*/"),
            ])
        } else {
            d.concat(&[
                d.text("//"),
                d.source_span(comment.content_span, self.source),
            ])
        };
        // The renderer records the emit when it reaches the node — see
        // `tsv_lang::comment_ledger`.
        #[cfg(feature = "comment_check")]
        d.tag_comment_doc(doc, comment.span, self.source);
        doc
    }

    /// Whether the source slice for `span` ends with a self-closing `/>` (for doc
    /// building). Shared by regular and special elements.
    pub(super) fn span_was_self_closing(&self, span: Span) -> bool {
        span.extract(self.source).trim_end().ends_with("/>")
    }
}
