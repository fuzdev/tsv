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

use crate::ast::internal::{self, FragmentNode, is_collapsible_ws_char};
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
}

/// The element-shaped inputs the shared analyze → layout → build pipeline reads.
///
/// [`internal::Element`] and [`internal::SpecialElement`] are distinct AST types that print
/// the same shape: a name, attributes, a fragment, and an open/close tag pair. Projecting
/// both onto one view lets the layout decisions (multiline-ness, boundary modes, hugging)
/// live in a single place. A `special_doc.rs` carrying its own hug predicates and its own
/// multiline decision drifts from this one (a `<slot>` that never goes multiline for block
/// children, a special path that dangles its delimiters where regular elements are
/// block-style).
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
    /// `<style>` / `<script>` — a raw-text element, and which one. `None` for every other
    /// tag. One field rather than two bools so "both" is unrepresentable and the tag's two
    /// consumers (the builder's dispatch, the sibling-dangle's exclusion) cannot drift.
    pub(super) raw_text: Option<RawTextKind>,
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
pub(in crate::printer) struct AttrGaps {
    /// Where the gap before the first attribute starts — the tag name's end.
    pub(in crate::printer) first_range_start: u32,
    /// The `>` closing the opening tag; bounds the gap after the last attribute.
    pub(in crate::printer) open_tag_end: u32,
    /// The comments inside the window that the caller prints itself, which the scan must
    /// therefore skip. `<svelte:element this={…}>` keeps its `this` out of the attribute
    /// list and synthesizes the attribute, so what it prints lands in the gaps probed here
    /// while the tag's own doc is what prints it; without the skip a comment in there is
    /// emitted twice, once by each. Ownership does not cover this on its own: a *glued block*
    /// comment is `owned_by_node` and already skipped on the `to emit` axis, but a line
    /// comment never is (`owned ⇒ is_block`).
    pub(in crate::printer) claimed: Option<ThisClaim>,
}

/// A built attribute list and the facts about its emission a caller may need before
/// printing the tag's closer — [`Printer::build_element_attrs_doc`]'s product, one value so
/// they cannot be separated and a flag silently dropped where it mattered.
///
/// Most callers read only `docs`: their `>` / `/>` already sits behind a `line` in the
/// list's own group, so the `break_parent` a line comment pushes moves it off the comment's
/// line unaided, and their attributes already wrap behind those same `line` separators. The
/// whitespace-sensitive builder is the one that must consult the emission — it hugs the `>`
/// onto the last attribute and holds its attributes flat, with no line to break for either
/// question (see [`Printer::push_attrs_with_comments`]).
pub(super) struct ElementAttrsDoc {
    pub(super) docs: DocBuf,
    pub(super) emission: AttrListEmission,
}

/// What emitting an attribute list did that a layout keeping the list **flat** must know —
/// [`Printer::push_attrs_with_comments`]'s summary of its comment runs.
///
/// Both fields are read off the emission as it happens rather than re-derived from source
/// or probed off the docs, so they cannot disagree with what was printed —
/// [`DocArena::will_break`](tsv_lang::doc::arena::DocArena::will_break) is *not* the
/// `has_hardline` question: it counts the `break_parent` a trailing `//` pushes, which
/// forces only the `>` off the comment's line while the list itself stays flat.
#[derive(Clone, Copy, Default)]
pub(in crate::printer) struct AttrListEmission {
    /// Whether the emitted list ends on a `//`, so nothing may share its line.
    pub(in crate::printer) ends_with_line_comment: bool,
    /// Whether a comment forced a hardline *into* the list — an own-line comment keeping
    /// its own line, or a same-line `//` pushing the following attribute to a fresh one.
    /// The list can no longer render flat at any width.
    pub(in crate::printer) has_hardline: bool,
}

/// What an emitted attribute-comment run leaves behind for whoever prints next.
///
/// Two questions, not one, and a caller that conflates them gets a different bug for each
/// direction: the separator question is about *any* comment that ends a line, while the
/// `>` question is about a `//` specifically — a `//` runs to end of line and would swallow
/// whatever shares it, where an own-line block comment is self-delimiting and the closer may
/// follow it inline. Both are read off the run as it is emitted rather than rescanned from
/// source, so the two answers cannot disagree with what was printed.
#[derive(Clone, Copy, Default)]
struct AttrCommentRun {
    /// Whether the next attribute must start on a fresh line — true for an own-line comment
    /// and for any line comment.
    next_on_new_line: bool,
    /// Whether the run's **last** comment is a `//`, so nothing may share its line.
    ends_with_line_comment: bool,
    /// Whether **any** comment in the run kept a line of its own (accumulated, unlike the
    /// two tail facts above): the run pushed a hardline, so the list holding it can never
    /// render flat — [`AttrListEmission::has_hardline`]'s per-run input.
    has_own_line_comment: bool,
}

/// Which comments the synthesized `this={…}` prints — [`AttrGaps::claimed`]'s payload, and
/// the single predicate ([`Self::claims`]) both sides of the seam ask: the attribute scan
/// skips exactly what the `this` site emits.
///
/// Not a contiguous span, because the binding may be **written after other attributes** yet
/// always prints first, and a comment must stay with the token it binds when the list is
/// reordered around it. A same-line comment trails the token before it; an own-line comment
/// leads the token after it (the same axioms the attribute-list emitters apply). Under the
/// hoist, exactly two positions therefore travel with `this`: a same-line comment in the
/// **tag-name gap** (it trails the tag name, the one token that never moves, so the head is
/// still its place) and an own-line comment in the **gap immediately before the binding**
/// (it leads `this`). Everything else stays in its source slot for the attribute scan — a
/// single `[name_end, value.end]` claim instead is how a comment trailing `data-x` got torn
/// off and re-anchored to `this`. When the binding is written first the regions coincide
/// and every window comment travels, which is the common authored form.
///
/// One deliberate approximation: a same-line comment *trailing an own-line comment* in the
/// tag-name gap travels while its predecessor stays. The pair splits once (each lands in a
/// stable slot, so the output is a fixed point); chain-aware anchoring would cost a
/// backward comment walk per query for a shape no authored code has produced.
#[derive(Clone, Copy)]
pub(in crate::printer) struct ThisClaim {
    /// The bound value as written — the `{…}` braces, or the plain form's text. Comments
    /// inside are printed by the binding's own doc.
    value: Span,
    /// End of the tag-name gap: the first source item's start (`value.start` when the
    /// binding is written first). A same-line comment ending by here trails the tag name.
    head_gap_end: u32,
    /// End of the last attribute written before the binding (the tag name's end when none
    /// is). An own-line comment at or after it leads the binding.
    prev_end: u32,
}

impl ThisClaim {
    pub(in crate::printer) fn new(
        name_end: u32,
        value: Span,
        attrs: &[internal::AttributeNode<'_>],
    ) -> Self {
        let head_gap_end = attrs
            .iter()
            .map(|a| a.span().start)
            .min()
            .map_or(value.start, |first| first.min(value.start));
        let prev_end = attrs
            .iter()
            .map(|a| a.span().end)
            .filter(|&end| end <= value.start)
            .max()
            .unwrap_or(name_end);
        Self {
            value,
            head_gap_end,
            prev_end,
        }
    }

    /// Start of the bound value as written — the end of the leading window whose claimed
    /// comments the `this` site emits itself (`[name_end, value_start)`). Comments past it
    /// that the claim covers sit *inside* the value and are printed by the binding's own doc.
    pub(in crate::printer) fn value_start(&self) -> u32 {
        self.value.start
    }

    /// Whether the synthesized `this` site prints `comment` — see the type docs for the
    /// routing rule.
    pub(in crate::printer) fn claims(&self, p: &Printer<'_>, comment: &tsv_lang::Comment) -> bool {
        if comment.span.start >= self.value.start {
            // At or past the value: claimed only inside it (the braces interior).
            return comment.span.end <= self.value.end;
        }
        if p.comment_starts_its_own_line(comment) {
            // Own-line: travels only when it leads the binding itself.
            self.prev_end <= comment.span.start
        } else {
            // Same-line: travels only when it trails the tag name.
            comment.span.end <= self.head_gap_end
        }
    }
}

/// Why an element's content lays out multiline — and whether that reason survives reformatting.
///
/// The distinction is load-bearing, not bookkeeping. [`BoundaryMode::Hard`] is exactly
/// "multiline", so a rule that reads the boundary mode reads this decision too — and a
/// [`Self::SourceBreaks`] decision is one tsv's **own output** rewrites, since converging an
/// authoring to block-style adds or removes exactly those newlines. A layout keyed on it can
/// therefore be re-decided on the next pass; a layout keyed on [`Self::Structural`] cannot.
/// [`Printer::handle_separator_text_child`]'s sibling-newline flow rule is the consumer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MultilineCause {
    /// Not multiline: the content collapses to one line, and width alone decides the layout.
    None,
    /// A property of the content itself forces it, however the source was authored — block
    /// children, mixed block/inline content, an expanding control-flow block, block flow, or a
    /// whitespace-collapsing container. Reformatting cannot change the answer.
    Structural,
    /// The content's own authored newlines forced it (`has_source_breaks_in_content`) — the
    /// Tier-2 element-expansion signal. Reformatting the content can add or remove those
    /// newlines, so this decision is not stable across passes.
    SourceBreaks,
}

impl MultilineCause {
    /// Whether the content lays out multiline at all — `Hard` boundary, block-style tags.
    pub(super) fn is_multiline(self) -> bool {
        self != Self::None
    }
}

/// Analysis context for element formatting decisions
///
/// Computed once per element from its [`ElementParts`], used to determine layout and build
/// docs. Strictly the *derived* half — anything readable straight off `ElementParts` (the tag
/// kind, void-ness) stays there, so no fact has two sources that could drift apart.
pub(super) struct ElementContext {
    /// Whether element was self-closing in source
    pub(super) is_self_closing: bool,
    /// Whether element has no meaningful content
    pub(super) is_empty: bool,
    /// Whether children need multiline formatting, and why — see [`MultilineCause`]
    pub(super) multiline: MultilineCause,
    /// Whether any attribute source contains embedded newlines (forces attr group break)
    pub(super) has_multiline_attr: bool,
}

/// Which raw-text element a `<script>` / `<style>` body belongs to.
///
/// The two differ in exactly two places — which language names their body may be written in
/// and still be formatted, and which parser formats it — so they are one code path with a
/// tag, not two, and the tag travels as a type rather than as a bool re-read at each of those
/// decisions (where a flipped test is a silent mis-format rather than a compile error).
///
/// Two variants, not three: this drives the **parser** dispatch below, which genuinely has
/// only two arms. The *language* question has a third position (`<template>`) and lives on
/// [`internal::EmbeddedLang`], which this maps into rather than re-deriving.
#[derive(Debug, Clone, Copy)]
pub(super) enum RawTextKind {
    Style,
    Script,
}

impl RawTextKind {
    /// This tag's position in the freeze rule — the owner of its formattable-name set. See
    /// docs/conformance_prettier_svelte.md §Foreign-language embedded bodies.
    fn lang(self) -> internal::EmbeddedLang {
        match self {
            Self::Style => internal::EmbeddedLang::Style,
            Self::Script => internal::EmbeddedLang::Script,
        }
    }
}

impl<'a> Printer<'a> {
    /// `<name>` — a whole start tag with no attributes (HTML spec "start tag").
    /// `name` is a pre-built name doc (span-identity `source_span`).
    ///
    /// The counterpart to [`Printer::end_tag`], and it marks the same distinction from the
    /// other side: an attribute-bearing tag goes through [`Printer::build_opening_tag`],
    /// which stops *before* the `>` so its caller can place that character — hug it to the
    /// last attribute, dedent it to its own line, or hand it to a sibling. Only where there
    /// is nothing to decide is the `>` part of the tag itself, and that is what this spells.
    #[inline]
    pub(super) fn start_tag(&self, name: DocId) -> DocId {
        let d = self.d();
        d.concat(&[d.text("<"), name, d.text(">")])
    }

    /// `</name>` — a whole end tag (HTML spec "end tag").
    ///
    /// ⚠️ **Every site that emits a complete end tag calls this**, so a remaining bare
    /// `d.text("</")` in this crate marks the other thing: an end tag whose `>` is placed
    /// *elsewhere* — dangled onto its own line, deferred to an enclosing group's break
    /// decision, or handed to a following sibling (the axis-3 `>` dangle). That distinction is
    /// load-bearing in this printer and invisible when both spellings are three anonymous
    /// `text` nodes, which is the whole reason this helper is worth calling for three tokens.
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
            raw_text: if facts.is_style() {
                Some(RawTextKind::Style)
            } else if facts.is_script() {
                Some(RawTextKind::Script)
            } else {
                None
            },
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
    pub(super) fn build_element_doc(&self, element: &internal::Element<'_>) -> DocId {
        let class = self.classify_tag(element);
        let is_html = element.kind == internal::ElementKind::Html;

        // Build attribute docs (needed for all paths)
        let attrs = self.build_element_attrs_doc(
            element.attributes,
            self.d().line(),
            element.name_span.end,
            element.open_tag_end,
            is_html,
        );

        // Special handling for <style> and <script> elements
        if let Some(kind) = class.raw_text {
            return self.build_raw_content_element_doc(kind, element, attrs.docs);
        }

        // Frozen-language <template> elements (e.g., <template lang="pug">)
        // preserve content raw — we can't format non-HTML template languages
        if element.is_frozen_template(self.source) {
            return self.build_frozen_template_doc(element);
        }

        // Whitespace-sensitive elements (pre, textarea, etc.) — these keep the mandatory
        // delimiter dangle, so they must never reach the shared layout analysis below.
        if class.is_ws_sensitive {
            return self.build_whitespace_sensitive_element_doc(element, attrs);
        }

        let attr_docs = attrs.docs;
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
    pub(super) fn build_inline_element_omit_close_gt(
        &self,
        element: &internal::Element<'_>,
    ) -> Option<DocId> {
        self.build_inline_element_sibling_gt(element, true, None)
    }

    /// Shared eligibility + setup for the two axis-3 sibling-`>` roles — the element→element/block
    /// follower ([`Self::build_inline_element_sibling_gt`]) and the glued-text follower
    /// ([`Self::build_inline_element_close_gt_dangle`]). Classifies the tag, builds its attrs /
    /// parts / context, and confirms the flat hug-both (`Soft`) content layout — the single shape
    /// with one trailing `>` we can cleanly split off. Returns `(parts, ctx, attr_docs,
    /// children_doc)`; `None` for any non-Soft shape (the callers then keep the element and its `>`
    /// intact).
    ///
    /// `children_doc` is the trimmed inline shape `build_content_element_doc`'s Soft arm builds, so
    /// a dangled element renders its content identically to its undangled form (incl. trimming a
    /// render-free boundary space — `<span>text </span>{#each…}` must dangle like the glued form).
    ///
    /// Special-content elements (raw `<script>`/`<style>`, frozen `<template>`, whitespace-sensitive
    /// `<pre>`/`<textarea>`) never participate — their closing tags aren't the simple hug-both shape.
    /// Soft alone qualifies (Hug's glued boundaries already collapse to it): a one-sided-newline or
    /// render-free boundary trims to the same glued form, so without the dangle `format(newline
    /// authoring)` would emit the glued no-dangle form, which the next pass reads as Hug and dangles —
    /// a non-idempotent 2-cycle (`authoring_audit`'s hard bucket). Multiline (Hard) and
    /// void/empty/self-closing forms keep their `>`.
    fn prepare_soft_sibling_element<'e>(
        &self,
        element: &'e internal::Element<'e>,
    ) -> Option<(ElementParts<'e>, ElementContext, DocBuf, DocId)> {
        let class = self.classify_tag(element);
        if class.raw_text.is_some()
            || class.is_ws_sensitive
            || element.is_frozen_template(self.source)
        {
            return None;
        }
        let is_html = element.kind == internal::ElementKind::Html;
        // Every layout below puts the `>` behind a line of this list's own group, so a
        // trailing `//`'s `break_parent` moves it off the comment's line unaided.
        let attr_docs = self
            .build_element_attrs_doc(
                element.attributes,
                self.d().line(),
                element.name_span.end,
                element.open_tag_end,
                is_html,
            )
            .docs;
        let parts = self.element_parts(element, class);
        let ctx = self.analyze_element(&parts, &attr_docs);
        match self.compute_element_layout(&parts, &ctx) {
            ElementLayout::WithContent(BoundaryMode::Soft) => {
                let children_doc =
                    self.build_nodes_doc_trimmed(element.fragment.nodes, MultilineCause::None);
                Some((parts, ctx, attr_docs, children_doc))
            }
            _ => None,
        }
    }

    /// Shared body for the axis-3 element sibling-`>` roles, composable so one element can play
    /// **both** at once inside a glued run (`build_glued_element_run`): it **sheds** its closing
    /// `>` to the following sibling (`external_close = true`) and/or **receives** a preceding
    /// sibling's `>` as a leading `if_break` inside its attrs group (`gt_prefix = Some`) — a mid-run
    /// element does both. `None` for any non-Soft shape (the boundary stays an intact `>`).
    pub(super) fn build_inline_element_sibling_gt(
        &self,
        element: &internal::Element<'_>,
        external_close: bool,
        gt_prefix: Option<DocId>,
    ) -> Option<DocId> {
        let (parts, ctx, attr_docs, children_doc) = self.prepare_soft_sibling_element(element)?;
        Some(self.build_collapsible_element_doc(
            &parts,
            &ctx,
            &attr_docs,
            children_doc,
            external_close,
            gt_prefix,
        ))
    }

    /// Build a glued-both inline element as the closing-`>` dangle onto glued following text —
    /// the axis-3 sibling-`>` dangle generalized from an element/block follower to a **text**
    /// follower (see fragment_doc's `try_build_glued_both_text_dangle`). Returns `Some` only for
    /// the flat hug-both (`Soft`) shape, mirroring [`Self::build_inline_element_sibling_gt`]'s
    /// eligibility; `None` keeps the element intact so the caller falls back to the normal path.
    ///
    /// A three-state `conditional_group` — the renderer picks the first whose flat first line fits:
    /// 1. **inline** `<span>content</span>` — the element fits fully on the line;
    /// 2. **dangle** `<span>content</span⏎>` — content stays inline, only the `>` drops to the
    ///    following text's line; chosen when `prefix<span>content</span` still fits. Render-safe:
    ///    the newline sits inside the end tag (ignored), so the AST is byte-identical;
    /// 3. **block-style** — the ordinary collapsible group (both tags intact, content on its own
    ///    indented line), the fallback when even `…</span` overflows (a wide prefix or wide
    ///    content), where dangling the `>` wouldn't help. Identical to the non-dangle output.
    ///
    /// The `inline`/`dangle` states share the `<span>content</span` head (only the final `>`'s
    /// placement differs), and `children_doc` is shared with `block_state` too — a `conditional_group`
    /// candidate that never renders never records its comments (the print-once ledger keys on the
    /// rendered node), so the shared subtrees are sound.
    pub(super) fn build_inline_element_close_gt_dangle(
        &self,
        element: &internal::Element<'_>,
    ) -> Option<DocId> {
        let (parts, ctx, attr_docs, children_doc) = self.prepare_soft_sibling_element(element)?;
        let d = self.d();
        let name = parts.name;
        let opening = self.build_opening_tag(name, &attr_docs, ctx.has_multiline_attr);
        let head = d.concat(&[opening, d.text(">"), children_doc, d.text("</"), name]);
        let inline_state = d.concat(&[head, d.text(">")]);
        let dangle_state = d.concat(&[head, d.hardline(), d.text(">")]);
        let block_state =
            self.build_collapsible_element_doc(&parts, &ctx, &attr_docs, children_doc, false, None);
        Some(d.conditional_group(&[inline_state, dangle_state, block_state]))
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
            // Always the attr-keyed trailing break. There is deliberately no `hug_start &&
            // !is_empty` fast path (emitting the attr concat alone, skipping an `empty()` child):
            // a hugged open tag suppressing the trailing break is exactly the delimiter-dangle
            // machinery the block-style stance excludes, and there is no `empty()` child to avoid.
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
        // A whitespace-collapsing container lays its children out one-per-line with the
        // inter-sibling whitespace trimmed (render-free — the compiler removes it). Its multiline
        // decision is forced (see `analyze_element`), so `boundary` is always `Hard` here and this
        // content flows into the multiline arm below.
        //
        // The multiline arm carries the *cause* (see [`MultilineCause`]), not just the fact:
        // `Hard` derived from the content's own authored newlines is a layout the next pass can
        // re-decide, which the sibling-newline flow rule has to know. `boundary` stays the source
        // of the multiline-ness itself, so a `Soft` boundary builds the inline arm as before.
        let children_doc = if parts.collapses_child_ws {
            self.build_container_content_doc(nodes)
        } else {
            let cause = if boundary == BoundaryMode::Hard {
                ctx.multiline
            } else {
                MultilineCause::None
            };
            self.build_nodes_doc_trimmed(nodes, cause)
        };

        // Soft boundaries: collapse when the element fits, break block-style when it doesn't.
        //
        // Always softlines: an authored boundary space is render-free (the compiler trims every
        // fragment edge), so it neither survives inline — `<span> text </span>` collapses to
        // `<span>text</span>` — nor selects the layout. Prettier instead keeps the space
        // (`printLineBeforeChildren`'s `line` when hasLeadingSpaces && isLeadingSpaceSensitive,
        // the HTML/CSS inline whitespace model Svelte 5 broke from) — see
        // conformance_prettier_svelte.md §Svelte: Inline content block-style and the
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
        let indent_inner = d.indent_hardline(children_doc);
        d.concat(&[
            opening_tag,
            d.text(">"),
            indent_inner,
            d.hardline(),
            self.end_tag(parts.name),
        ])
    }

    /// Build doc for the collapsible (`Soft`) content layout — the single inline shape, whatever
    /// the author wrote at the boundary.
    ///
    /// Softline boundaries: the content collapses onto the tag line when it fits and drops to its
    /// own indented line (block-style, both tags intact) when it doesn't. Since the boundary run
    /// is render-free and always trimmed, a glued authoring and a spaced one reach this same
    /// builder — that is what makes them converge. No hardline force is needed — every multiline
    /// trigger (an expanding control-flow block, block-flow children, any other [`MultilineCause`])
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
        // longer flows on the tag lines. See conformance_prettier_svelte.md.
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
    /// conformance_prettier_svelte.md §Svelte: Inline content block-style). When attrs force
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

            // State 2: Hug mode - attrs inline (space-separated), > on new line.
            // The `>` takes a hardline of its own here, so a trailing `//` cannot swallow it.
            let hug_attrs = self
                .build_element_attrs_doc(
                    element.attributes,
                    self.d().text(" "),
                    element.name_span.end,
                    element.open_tag_end,
                    is_html,
                )
                .docs;
            let hug_state = d.concat(&[
                d.text("<"),
                name_doc,
                d.concat(&hug_attrs),
                d.hardline(),
                closing,
            ]);

            // State 3: Full multiline - attrs on separate lines, > on new line
            let multiline_attrs = self
                .build_element_attrs_doc(
                    element.attributes,
                    self.d().line(),
                    element.name_span.end,
                    element.open_tag_end,
                    is_html,
                )
                .docs;
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

    /// Build a doc for a `<template>` element in a frozen language (e.g., `lang="pug"`).
    /// Content is preserved raw — we can't format non-HTML template languages.
    /// Format: `<template lang="pug">\n{raw content}\n</template>`
    fn build_frozen_template_doc(&self, element: &internal::Element<'_>) -> DocId {
        let d = self.d();
        let name_doc = d.source_span_ident(element.name_span);

        // Opening tag: the ordinary one. What is frozen here is the element's CONTENT, which
        // tsv cannot format and so copies verbatim; the head above it is an attribute list like
        // any other and gets the shared layout — attributes wrapped one per line and the `>` at
        // base indent once it breaks. Answering that here instead cost both halves of the
        // question: the hand-rolled concat had no group, so a 128-column head never wrapped,
        // and no line before the `>`, so a trailing `//` swallowed it along with the whole
        // template body and the output stopped re-parsing. `build_opening_tag` ends in a
        // dedented softline, which the `break_parent` a line comment pushes expands unaided.
        // Foreign template elements are always HTML, so is_html=true.
        let attr_docs = self
            .build_element_attrs_doc(
                element.attributes,
                d.line(),
                element.name_span.end,
                element.open_tag_end,
                true,
            )
            .docs;
        let mut parts: DocBuf = smallvec![self.build_opening_tag(name_doc, &attr_docs, false)];
        parts.push(d.text(">"));

        // The body is in a frozen language, so it is the author's own bytes: the whole child
        // run's source span rides out verbatim, boundary newlines aside. Reading the fragment
        // node by node instead cost every non-`Text` child — an element, a comment, an
        // expression tag, a block were all silently DELETED, and no self-oracle gate could see
        // it: the truncated output is its own fixed point, reparses, and drops no comment the
        // ledger tracks. Prettier says the same thing structurally (`printRaw` +
        // `preformattedBody`): the span runs first child start → last child end, one leading
        // blank-through-newline and one trailing newline-through-blanks are stripped, and the
        // body sits between a `literalline` (so the first line keeps column 0 — the author's
        // indentation is part of a whitespace-significant language) and a `hardline` (so the
        // closing tag takes the element's own indent). An empty fragment gets no body and no
        // newlines at all: `<template lang="pug"></template>`.
        let nodes = element.fragment.nodes;
        if let (Some(first), Some(last)) = (nodes.first(), nodes.last()) {
            parts.push(self.frozen_body_doc(Span::new(first.span().start, last.span().end)));
        }

        parts.push(self.end_tag(name_doc));

        d.concat(&parts)
    }

    /// Build a doc for a nested `<style>` or `<script>` element with formatted CSS/JS content
    ///
    /// This handles nested style/script elements (inside other elements like `<div>`)
    /// that need their content formatted as CSS/JS rather than as regular fragment nodes.
    pub(super) fn build_raw_content_element_doc(
        &self,
        kind: RawTextKind,
        element: &internal::Element<'_>,
        attr_docs: DocBuf,
    ) -> DocId {
        let d = self.d();
        let name_doc = d.source_span_ident(element.name_span);
        // The attr-keyed opening tag every other element path takes — a hand-rolled copy of
        // it here (same softline, same dedent, same indent) would be the two-copies-drift
        // this crate's `ElementParts` doc warns about, one tag lower down.
        // The group wraps `build_opening_tag`'s output plus the `>` the helper deliberately
        // leaves to its caller (see [`Printer::end_tag`] for the same split at the other tag).
        let opening_tag = d.group(d.concat(&[
            self.build_opening_tag(name_doc, &attr_docs, false),
            d.text(">"),
        ]));
        // Every arm below ends with it, so it is built once rather than at each of the five.
        let closing_tag = self.end_tag(name_doc);

        // Get raw content from the single Text child
        let text = element.fragment.nodes.first().and_then(|node| match node {
            FragmentNode::Text(text) => Some(text),
            _ => None,
        });

        // Nothing between the tags — the one arm that collapses. A body of *whitespace* is
        // not this arm: it keeps a delimiter break at every other position and here too.
        let Some(text) = text.filter(|t| !t.raw(self.source).is_empty()) else {
            return d.concat(&[opening_tag, closing_tag]);
        };

        // A frozen-language body freezes before any parse is attempted — the shared
        // opacity gate, asked here at both nested positions exactly as at the two top-level
        // ones (`print_style` / `print_script`), and answered with the one freeze shape:
        // the author's bytes verbatim. Unlike the top-level positions, nested content is raw
        // text to BOTH parsers (canonical Svelte never parses it), so nothing here has
        // established anything about the body — not even that it is brace-structured. That
        // is precisely why the freeze may not re-indent it: `lang="sass"` and `lang="stylus"`
        // are indentation-significant, and shifting such a body off column 0 changes what it
        // says, the same corruption in miniature as reprinting a less body with the CSS
        // printer (`@color: red;` → `@color : red;`).
        if kind.lang().is_frozen(element.attributes, self.source) {
            // `raw_span`, not `span`: what rides out is the author's own bytes. The two
            // coincide on a raw-text `Text` (no decode), and naming the raw one says so.
            return d.concat(&[
                opening_tag,
                self.frozen_body_doc(text.raw_span),
                closing_tag,
            ]);
        }

        let content = text.data(self.source);

        // A formattable body with nothing to format still holds its delimiter break — the two
        // tags with one break between them, the closing tag back at the element's own indent.
        // That is the shape `print_script` / `print_style` give it one level up, and
        // prettier's `content === '' ? '' : hardline` arm. It is the shape a
        // **whitespace-only** body takes, not an empty one, which collapsed above — the
        // frozen twin keeps its own whitespace instead (`preformattedBody('')` returns the
        // empty doc where `preformattedBody('   ')` returns the literal-line pair), which is
        // why this question is asked below the freeze gate rather than above it.
        if content.trim().is_empty() {
            return d.concat(&[opening_tag, d.hardline(), closing_tag]);
        }

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
        let formatted = match kind {
            RawTextKind::Style => tsv_css::parse(&content, &arena)
                .ok()
                .map(|ast| tsv_css::format_in(&ast, &content, self.d())),
            RawTextKind::Script => tsv_ts::parse(&content, &arena)
                .ok()
                .map(|ast| tsv_ts::format_in(&ast, &content, self.d())),
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
                d.concat(&[opening_tag, indented, d.hardline(), closing_tag])
            }
            _ => {
                // Fallback: preserve raw content. Reachable only for a FORMATTABLE-lang
                // body (css/ts/absent) — a frozen body froze before the parse above — and
                // it catches TWO cases, not one: a body whose content doesn't parse
                // (`None`), and a body that parses but formats to EMPTY (`Some` whose trim
                // is empty — `<script>;</script>`, guarded out by the arm above).
                // TODO: route through the freeze emitter for a cleaner shape? For the
                // parse-fail half prettier has only its degraded error-swallow path (no
                // clean oracle to pin a fixture against); the formats-to-empty half DOES
                // have one — prettier opens the pair around a blank line where this arm
                // glues the raw one-liner — see
                // docs/conformance_prettier_svelte.md §Foreign-language embedded bodies.
                d.concat(&[opening_tag, d.text_pooled(&content), closing_tag])
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
    pub(super) fn build_element_attrs_doc(
        &self,
        attrs: &[internal::AttributeNode<'_>],
        separator: DocId,
        name_end: u32,
        open_tag_end: u32,
        is_html: bool,
    ) -> ElementAttrsDoc {
        // Most elements have a handful of attributes, so the per-element parts
        // buffer stays on the stack (`DocBuf`'s inline capacity); attribute-dense
        // elements spill to the heap as before.
        let mut docs: DocBuf = DocBuf::with_capacity(attrs.len() * 2);
        let emission = self.push_attrs_with_comments(
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
        ElementAttrsDoc { docs, emission }
    }

    /// Push attribute docs with interleaved JS comment handling.
    ///
    /// The one attribute-list emitter: regular elements, special elements, and the hoisted
    /// `<svelte:options>` head all come through here. Handles comments before each attribute
    /// and after the last one — or, when the list is empty, the whole window — over the range
    /// described by [`AttrGaps`].
    ///
    /// Returns the [`AttrListEmission`] summary, which is what a caller keeping the list or
    /// its closer **flat** must know. Most callers need neither field: their `>` already
    /// sits behind a `line` in this list's own group, so the `break_parent` a line comment
    /// pushes puts it on the next line by itself, and their attributes already wrap behind
    /// those same `line` separators. The whitespace-sensitive builder — where a hug is a
    /// refusal to inject rendered whitespace — has no such line for either question and
    /// must ask.
    pub(in crate::printer) fn push_attrs_with_comments(
        &self,
        docs: &mut DocBuf,
        attrs: &[internal::AttributeNode<'_>],
        separator: DocId,
        gaps: AttrGaps,
        is_html: bool,
    ) -> AttrListEmission {
        let AttrGaps {
            first_range_start,
            open_tag_end,
            claimed,
        } = gaps;
        // The gap probes below all go through this, so the claim is honored once here
        // rather than at each of the sites.
        let gap_comments = |start: u32, end: u32| {
            comments_to_emit_in_range(self.comments, start, end)
                .filter(move |c| !claimed.is_some_and(|cl| cl.claims(self, c)))
        };
        let has_gap_comments = |start: u32, end: u32| gap_comments(start, end).next().is_some();
        // Where the gap *before* attribute `i` starts: the previous attribute's end, or the
        // window start when there is no previous one. Asked of `attrs.len()` it is the gap
        // after the last attribute — and, for an empty list, the whole window. One rule for
        // every gap, so the trailing range cannot be anchored differently from the rest:
        // guarding that one on `attrs.last()` instead is exactly how an attribute-less tag
        // (`<div // c⏎>`, `<Comp /* c */ />`) silently deleted every comment in its head, and
        // one real attribute was enough to hide it.
        let gap_start = |i: usize| {
            i.checked_sub(1)
                .map_or(first_range_start, |prev| attrs[prev].span().end)
        };

        // Every gap this fn probes lies inside `[first_range_start, open_tag_end]`. A comment
        // lands in a probe only when it sits fully inside the queried range, so a
        // comment-free open tag means every one of those gaps is comment-free: each would
        // reach `push_attr_item_with_leading_comments` with an empty run and take its
        // bare-separator branch, and the trailing block would emit nothing. Answer that with
        // one probe instead of one per attribute plus one. (Whole-window and per-gap probes
        // share `gap_comments`, so the claim is honored identically by both — a claim that
        // shortcut only the per-gap probes would re-open the double-print through this fast
        // path.)
        if !has_gap_comments(first_range_start, open_tag_end) {
            for attr in attrs {
                docs.push(separator);
                docs.push(self.build_attribute_node_doc(attr, is_html));
            }
            return AttrListEmission::default();
        }

        let mut has_hardline = false;
        for (i, attr) in attrs.iter().enumerate() {
            has_hardline |= self.push_attr_item_with_leading_comments(
                docs,
                separator,
                gap_comments(gap_start(i), attr.span().start),
                self.build_attribute_node_doc(attr, is_html),
            );
        }

        // The gap after the last attribute — or, for an empty list, the whole window, where
        // the loop above ran zero times and this is the tag's only emitter. An empty run is
        // a no-op, so it needs no guard.
        let trailing =
            self.push_attr_comment_docs(docs, gap_comments(gap_start(attrs.len()), open_tag_end));
        AttrListEmission {
            ends_with_line_comment: trailing.ends_with_line_comment,
            has_hardline: has_hardline || trailing.has_own_line_comment,
        }
    }

    /// Push one attribute-list item behind whatever comments lead it.
    ///
    /// The item takes `separator` either way; a comment run only ever *upgrades* it to a
    /// hardline, when the run ends on its own line or on any line comment (a `//` runs to end
    /// of line). A same-line block comment upgrades nothing — the run trails the token before
    /// it and the item goes on taking the list's own separator, which is a space in a flat
    /// layout and a break in a wrapped one.
    ///
    /// Hard-coding that space instead is how a same-line block comment came to *weld* the
    /// following attribute onto the preceding line — `data-a="1" /* c */ data-b="2"` sitting
    /// on one line inside a list that is otherwise one attribute per line, and the comment
    /// bound to the attribute after it rather than the token it was written after. Flat
    /// layouts hid it completely, since there `separator` renders as that same space. Pinned
    /// by [`comment_same_line_long`](../../../../../tests/fixtures/svelte/attributes/comment_same_line_long_prettier_divergence/).
    ///
    /// One seam for two callers, because it is one decision: the attribute loop in
    /// [`Self::push_attrs_with_comments`], and the synthesized `this={…}` that
    /// `build_special_element_attrs_doc` prints before that loop runs. Keeping a second copy
    /// beside the `this` binding is how the two drifted in the first place — that one printed
    /// no leading comments at all, so every comment before the binding was dropped.
    ///
    /// Returns whether the emission put a hardline into the list — a comment in the run kept
    /// its own line, or the run's tail forced the item onto a fresh one
    /// ([`AttrListEmission::has_hardline`]'s per-item input).
    pub(super) fn push_attr_item_with_leading_comments<'c>(
        &self,
        docs: &mut DocBuf,
        separator: DocId,
        comments: impl IntoIterator<Item = &'c tsv_lang::Comment>,
        item: DocId,
    ) -> bool {
        let d = self.d();
        let mut comments = comments.into_iter().peekable();
        let pushed_hardline = if comments.peek().is_none() {
            docs.push(separator);
            false
        } else {
            let tail = self.push_attr_comment_docs(docs, comments);
            docs.push(if tail.next_on_new_line {
                d.hardline()
            } else {
                separator
            });
            tail.has_own_line_comment || tail.next_on_new_line
        };
        docs.push(item);
        pushed_hardline
    }

    /// Whether the author put `comment` on a line of its own — the question every
    /// attribute-list gap emitter asks to pick the comment's separator.
    ///
    /// Answered by scanning **backwards** from the comment over inter-token whitespace
    /// ([`is_collapsible_ws_char`], shared rather than restated — anything outside that class
    /// is a token, and stopping there is the answer): it starts its own line when a newline is
    /// crossed first. The obvious alternative — "does `source[gap_start..comment_start]`
    /// contain a newline?" — asks a *different* question wherever the printer emits something
    /// into that span, and gets it wrong: `<svelte:element>` synthesizes `this={…}` from the
    /// element kind rather than from `attributes`, so a comment after the binding is measured
    /// from the **tag name** and, once the tag has broken across lines, the scan finds the
    /// printer's own newline and moves the comment to its own line — a second pass that
    /// changes the output. Asking about the comment alone has no anchor to get wrong.
    fn comment_starts_its_own_line(&self, comment: &tsv_lang::Comment) -> bool {
        self.source[..comment.span.start as usize]
            .chars()
            .rev()
            .take_while(|&c| is_collapsible_ws_char(c))
            .any(|c| c == '\n')
    }

    /// Push docs for JS comments between attributes.
    ///
    /// Each comment gets a preceding separator (hardline when it starts its own
    /// line, an inline space when it trails the previous token). Returns what the
    /// emitted run leaves behind for whoever prints next — see [`AttrCommentRun`].
    fn push_attr_comment_docs<'c>(
        &self,
        docs: &mut DocBuf,
        comments: impl IntoIterator<Item = &'c tsv_lang::Comment>,
    ) -> AttrCommentRun {
        let d = self.d();
        let mut tail = AttrCommentRun::default();
        for comment in comments {
            let is_own_line = self.comment_starts_its_own_line(comment);

            // Preserve the author's placement: a comment on its own line stays on its
            // own line; a comment on the same line as the preceding token stays
            // trailing it (inline). Block and line comments alike (a `//` the author
            // put after the tag name or an attribute is kept there rather than
            // relocated to its own line).
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
            tail = AttrCommentRun {
                next_on_new_line: is_own_line || !comment.is_block,
                ends_with_line_comment: !comment.is_block,
                has_own_line_comment: tail.has_own_line_comment || is_own_line,
            };
        }
        tail
    }

    /// Build a doc for a JS comment's text (without surrounding separators).
    ///
    /// The bare `Printer::js_comment_text_doc` spelling plus the ledger tag — this builder
    /// adds no separator and no break of its own; the caller
    /// ([`Self::push_attr_comment_docs`]) supplies both.
    pub(super) fn build_attr_js_comment_doc(&self, comment: &tsv_lang::Comment) -> DocId {
        let doc = self.js_comment_text_doc(comment);
        // The renderer records the emit when it reaches the node — see
        // `tsv_lang::comment_ledger`.
        #[cfg(feature = "comment_check")]
        self.d().tag_comment_doc(doc, comment.span, self.source);
        doc
    }

    /// Whether the source slice for `span` ends with a self-closing `/>` (for doc
    /// building). Shared by regular and special elements.
    pub(super) fn span_was_self_closing(&self, span: Span) -> bool {
        span.extract(self.source).trim_end().ends_with("/>")
    }
}
