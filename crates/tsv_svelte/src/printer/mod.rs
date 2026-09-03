// Svelte printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Core Printer struct and root-level printing orchestration
// - **nodes/**: Node-specific printing (elements, expressions, control flow, etc.)
// - **text.rs**: Text-analysis predicates (leading/trailing whitespace, blank lines)
// - **script_style.rs**: Script and style section printing
// - **attributes.rs**: HTML attribute and directive printing
// - **classification/**: HTML element classification adapters
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier-plugin-svelte for compatibility
// 2. **Preserve Semantics**: Never change HTML whitespace rendering semantics
// 3. **Source Layout**: Preserve authorial intent via inline run grouping
// 4. **Modularity**: Each module has single responsibility for future maintainability

mod attributes;
mod classification;
mod frozen_body;
mod helpers;
mod nodes;
mod script_style;
mod text;

use crate::ast::internal::{self, FragmentNode, is_collapsible_ws_char};
use nodes::AttrGaps;
use smallvec::SmallVec;
use std::cell::{Cell, RefCell};
use tsv_lang::FxHashSet;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::printing::LineBreaks;
use tsv_lang::{
    Comment, EmbedContext, INDENT, LayoutMode, OutputBuffer, Span, TAB_WIDTH,
    comments_in_source_range, comments_to_emit_in_range, is_format_ignore_directive,
    is_format_ignore_range_end, is_format_ignore_range_start, is_honored_format_ignore,
};
use tsv_ts::Expression;

/// A buffered run of comments from one gap — collected rather than iterated because the
/// callers ask two questions of it (how does each one lay out? and how did the run end?).
/// Mirrors `tsv_ts`'s `CommentVec`, which this crate can't see (`pub(crate)` there).
pub(in crate::printer) type CommentRun<'a> = SmallVec<[&'a Comment; 8]>;

/// Which section a fragment comment should travel with during canonical reordering.
/// Comments attach to the nearest section that follows them in source order.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CommentSection {
    Options,
    ModuleScript,
    InstanceScript,
    Template,
    Style,
}

/// The four root sections in canonical print order, each with the [`CommentSection`] it
/// anchors. **The single enumeration of what hoists**: the comment-classification table
/// ([`Printer::classify_fragment_comment`]) and the range-slice cuts
/// ([`Printer::build_ignore_range_doc`], via `print_component`'s `hoisted`) both read it, so a
/// section kind cannot join one and silently miss the other — a kind the cut missed would
/// re-open the duplicate-section emit for exactly that kind.
fn root_sections(root: &internal::Root<'_>) -> [(Option<Span>, CommentSection); 4] {
    [
        (
            root.options.as_ref().map(|s| s.span),
            CommentSection::Options,
        ),
        (
            root.module.as_ref().map(|s| s.span),
            CommentSection::ModuleScript,
        ),
        (
            root.instance.as_ref().map(|s| s.span),
            CommentSection::InstanceScript,
        ),
        (root.css.as_ref().map(|s| s.span), CommentSection::Style),
    ]
}

/// A format-ignore **range marker**'s kind — the `…-start` / `…-end` pair that brackets a
/// frozen template region.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RangeMarker {
    Start,
    End,
}

/// Classify `comment` as a format-ignore range marker; `None` for every ordinary comment.
///
/// The one spelling of the marker test — the freeze scan, the comment-classification barrier,
/// and [`Printer::is_inside_ignore_range`] all ask it here, so they cannot drift on which
/// comments count as markers. A free function (not a method) because
/// [`Printer::print_root_fragment`]'s closures capture `source` alone to stay borrow-clean.
fn range_marker(comment: &internal::HtmlComment, source: &str) -> Option<RangeMarker> {
    let content = comment.content(source);
    if is_format_ignore_range_start(content) {
        Some(RangeMarker::Start)
    } else if is_format_ignore_range_end(content) {
        Some(RangeMarker::End)
    } else {
        None
    }
}

/// A head's content doc plus whether that content **opens on its own line** — the pair every
/// head builder hands back, and the one argument [`Printer::build_prefixed_head_doc`]
/// assembles from.
///
/// **The rationale every head builder shares, stated once here.** The verdict comes back
/// OUT of the builder rather than going in, because it selects the head's whole layout
/// (space-less prefix, no tail hug, already broken) and those parts must never disagree —
/// a hugged one would pull the run flush against the `{`. Threading them as separate values
/// let a caller pass one and forget the other; as one value there is nothing to forget, and
/// a caller that can't supply the flag can't supply a wrong one.
///
/// **Two things open a head's content on its own line, and the layout is the same for both**
/// ([`Printer::head_layout`]):
///
/// - a **freeze** — flush against the `{` a directive sits in a placement the floor calls
///   inert, so the freeze it earned would be gone on the next pass;
/// - an **own-line leading `//`** — the comment leads the head's VALUE, so where the author
///   put it is authoring signal (conformance_prettier.md §Comment Position Philosophy) and
///   pulling it up onto the prefix's line would relocate it.
///
/// The first is why the field existed; the second is the ordinary form the freeze turned
/// out to be a special case of.
///
/// `doc` is the head's content in its **final shape** — an own-line one is already broken onto
/// its own indented lines ([`Printer::indent_own_line_head`]), because the block heads consume
/// it directly rather than through the prefixed-head assembler.
///
/// `ends_with_line_comment` records that the content's last emitted comment was a **line**
/// comment, whose doc ends in a `hardline` that already drops the closing token to the next
/// line. Every consumer that would otherwise supply its own break must skip it, or the two
/// breaks stack into a blank line — the three closer assemblers
/// ([`Printer::build_prefixed_head_doc`], `wrap_in_block_structure`,
/// `build_block_head`) all read it for exactly that. It is set by
/// [`Printer::trailing_comment_docs`] off the run it emits rather than re-derived from
/// source by each consumer, because the builder that *emitted* the run is the one that knows.
///
/// `owes_continuation_indent` is the one field that qualifies `doc`'s final-shape claim, and
/// only for the head whose *caller* chooses the layout ([`Printer::build_expression_content_with_comments`],
/// the unprefixed `{…}`). It says a leading line comment hangs this content one level in
/// ([`HeadLayout::HangsAfterOpen`]) and the builder could not apply the indent
/// itself, because its caller may already supply one: an assembler that block-wraps the
/// content (`wrap_in_block_structure`) has paid the debt, and one that hugs its braces has
/// not. Every builder that owns its own assembly applies the indent in place and returns
/// `false` — the debt is always settled exactly once. `false` for an own-line head, whose
/// [`Printer::indent_own_line_head`] is the same indent one level up.
#[derive(Clone, Copy)]
pub(in crate::printer) struct HeadExpr {
    pub(in crate::printer) doc: DocId,
    pub(in crate::printer) layout: HeadLayout,
    pub(in crate::printer) ends_with_line_comment: bool,
    pub(in crate::printer) owes_continuation_indent: bool,
}

/// How a braced head's content sits between its delimiters — the three states
/// [`Printer::head_layout`] resolves, as one value rather than two bools, because the fourth
/// combination (the opening literal sheds its space but the closer hugs) is not a shape.
///
/// The two hanging arms differ in **one** thing, and deliberately only that: where the run's
/// first comment sits. Everything downstream of it — the content's indent, the dangling
/// closer, the [`Printer::trailing_comment_docs`] `closer_owns_break` answer — is shared, so
/// the two authorings of one head produce the same geometry around a comment in two places.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::printer) enum HeadLayout {
    /// Nothing in the gap forces a break: the content stays on the head's line and the closer
    /// hugs it (`{@html expr}`).
    Inline,
    /// A `//` the author wrote **after the opening literal**: it keeps that line, the value
    /// hangs one level in, and the closer drops to the head's own column
    /// (`{@html // c⏎\texpr⏎}`).
    HangsAfterOpen,
    /// The content opens on its **own** line — a freeze, or a `//` the author put on its own
    /// line. As `HangsAfterOpen`, plus: the opening literal sheds its trailing space (the
    /// content supplies its own hardline) and the whole run hangs with the value
    /// (`{@html⏎\t// c⏎\texpr⏎}`).
    OpensOwnLine,
}

impl HeadLayout {
    /// Whether the content begins on a line of its own — the opening literal sheds its
    /// trailing space, and an unprefixed `{` needs no separator before the run.
    pub(in crate::printer) const fn opens_own_line(self) -> bool {
        matches!(self, Self::OpensOwnLine)
    }

    /// Whether anything **indents** this content — which is the same question as whether the
    /// closer drops to its own line, and as [`Printer::trailing_comment_docs`]'s
    /// `closer_owns_break`. One accessor so a caller cannot answer the three differently.
    pub(in crate::printer) const fn indents_content(self) -> bool {
        !matches!(self, Self::Inline)
    }
}

/// Printer state for building output
pub(crate) struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Embedding context (layout mode, offsets)
    embed: EmbedContext,
    /// Arena allocator for doc nodes (borrowed so a multi-file driver can reuse
    /// one arena across files; see [`DocArena::reset`]).
    pub(crate) arena: &'a DocArena,
    /// Source code (needed for preserving whitespace semantics)
    pub(crate) source: &'a str,
    /// Comments from scripts and template expressions
    comments: &'a [Comment],
    /// Whether any of `comments` is owned by a node (`owned_by_node`). Computed once
    /// per document at construction and handed to `tsv_ts` via `ts_inputs()`, so the
    /// embedded owned-comment path short-circuits per `{expr}` without an O(comments)
    /// rescan there. `owned_by_node` is set during the eager parse of embedded TS, so
    /// it is already final before printing.
    has_owned_comments: bool,
    /// Whether any of `comments` is a `format-ignore` directive. Computed once per document
    /// at construction and handed to `tsv_ts` via `ts_inputs()`, so the embedded
    /// `member_gap_frozen` short-circuits per `{expr}` without an O(comments)
    /// rescan there — the same per-`{expr}` trap `has_owned_comments` documents.
    has_format_ignore: bool,
    /// The document's line-break table with its verdict (`tsv_lang::printing::LineBreaks`):
    /// the verdict is taken once at construction, the table is filled only if a line
    /// question falls back to it, and every embedded island borrows it through
    /// [`Self::line_table`].
    line_breaks: LineBreaks<'a>,
    /// Whether a wrapped block-tag head may dangle its `}` (and, later, expand its
    /// body) in the current context. True almost everywhere — including inside
    /// inline elements / components, where the body-expand is render-safe because a
    /// block's *body-boundary* whitespace is non-significant (the sibling boundary,
    /// e.g. `</span>{#if …}`, stays hugged regardless, since the expand never injects
    /// whitespace there). Set false only while building the content of a
    /// whitespace-significant element (`<pre>` / `<textarea>`), where every injected
    /// whitespace would render. Save/restore discipline:
    /// `build_whitespace_sensitive_content_doc` sets it false for its children and
    /// restores the previous value on the way out (so nested contexts reset
    /// correctly).
    block_dangle_allowed: Cell<bool>,
    /// Span starts of control-flow blocks the root fragment marked as part of a **single-line
    /// inline run** (`{x}{#if c}…{/if}` with no newline). The unified
    /// [`Printer::build_nodes_doc_multiline`] builds these in inline context (long body
    /// inner-breaks) rather than multiline context (body drops to its own line), reproducing
    /// the root's pre-unification `build_nodes_doc` layout (the load-bearing
    /// single-line-run discriminator). Span-keyed, so it scopes to **root-level** runs only —
    /// element-nested blocks (different spans) keep the multiline body-drop divergence (e.g.
    /// `blocks/await/preceding_sibling_body_long`). Populated once by
    /// [`Printer::mark_root_inline_run_blocks`] before the root content is built.
    /// Hashed by `tsv_lang`'s `FxHasher` rather than SipHash — only
    /// `insert`/`contains`/`clear` are used, never iteration, so the hasher is
    /// unobservable (see `tsv_lang::hash`'s module docs).
    root_inline_run_block_starts: RefCell<FxHashSet<u32>>,
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given source and comments (standalone layout).
    pub(crate) fn new(arena: &'a DocArena, source: &'a str, comments: &'a [Comment]) -> Self {
        Self::with_embed(arena, source, comments, EmbedContext::default())
    }

    /// Create a new printer with the given source, comments, and embed context.
    pub(crate) fn with_embed(
        arena: &'a DocArena,
        source: &'a str,
        comments: &'a [Comment],
        embed: EmbedContext,
    ) -> Self {
        // The document's one whole-source line table: every embedded island borrows
        // it (`build_program_doc` for `<script>`/`{expr}` TS, `tsv_css::format_embedded_in`
        // for `<style>` CSS) — never re-classify per island. Its table, if a line
        // question ever falls back to it, fills the arena-parked scratch (one warm table
        // across a multi-file driver's files); `into_string` parks it back.
        let line_breaks = LineBreaks::new(source, arena.take_line_breaks_scratch());
        // The two document-level presence flags come from the one scan `tsv_ts` owns
        // (`PrinterInputs::for_document`); `ts_inputs()` copies them per island.
        let tsv_ts::PrinterInputs {
            has_owned_comments,
            has_format_ignore,
            ..
        } = tsv_ts::PrinterInputs::for_document(source, comments, line_breaks.table());
        Self {
            buffer: OutputBuffer::with_capacity(source.len()),
            indent_level: 0,
            embed,
            arena,
            source,
            comments,
            has_owned_comments,
            has_format_ignore,
            line_breaks,
            block_dangle_allowed: Cell::new(true),
            root_inline_run_block_starts: RefCell::new(FxHashSet::default()),
        }
    }

    /// Whether `node` is a control-flow block the root marked as part of a single-line inline
    /// run — see [`Printer::root_inline_run_block_starts`]. Read by `build_nodes_doc_multiline`
    /// to build the block in inline (inner-break) rather than multiline (body-drop) context.
    pub(crate) fn is_root_inline_run_block(&self, node: &FragmentNode<'_>) -> bool {
        self.root_inline_run_block_starts
            .borrow()
            .contains(&node.span().start)
    }

    /// Get a reference to the doc arena (convenience for `self.arena`).
    #[inline]
    pub(crate) fn d(&self) -> &DocArena {
        self.arena
    }

    /// Whether a wrapped block-tag head may dangle its `}` in the current context.
    /// See [`Printer::block_dangle_allowed`] for the save/restore discipline.
    #[inline]
    pub(crate) fn block_dangle_allowed(&self) -> bool {
        self.block_dangle_allowed.get()
    }

    /// Set [`Printer::block_dangle_allowed`] to `allowed`, returning the previous
    /// value for the caller to restore. Used by the whitespace-sensitive element
    /// builder to gate the dangle off while building `<pre>` / `<textarea>` content.
    #[inline]
    pub(crate) fn set_block_dangle_allowed(&self, allowed: bool) -> bool {
        self.block_dangle_allowed.replace(allowed)
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Write `span` of the source **verbatim** to the buffer.
    ///
    /// The format-ignore seam for a whole `<script>` / `<style>` section: the island's
    /// comments (which `Root.comments` holds) ride out inside the raw slice and never
    /// reach an emitter, so the ledger is told the range is covered. The doc-side
    /// verbatim seams use [`Self::verbatim_source_doc`].
    pub(crate) fn write_verbatim_span(&mut self, span: Span) {
        #[cfg(feature = "comment_check")]
        tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);

        self.write(span.extract(self.source));
    }

    /// A doc emitting `span` of the source **verbatim** — the doc-side twin of
    /// [`Self::write_verbatim_span`] (a format-ignored template node, a format-ignore
    /// range).
    pub(crate) fn verbatim_source_doc(&self, span: Span) -> DocId {
        #[cfg(feature = "comment_check")]
        tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);

        // `verbatim_source_span`, not `source_span`: a format-ignored slice's
        // embedded newlines are source layout, opaque to `will_break`.
        self.d().verbatim_source_span(span, self.source)
    }

    /// A doc emitting `span` of the source as an ordinary [`DocId`] source slice —
    /// content, not a format-ignore freeze (an interior newline still breaks the
    /// enclosing group, unlike [`Self::verbatim_source_doc`]) — while telling the
    /// ledger that any comment inside rides out in the slice. For the emitters
    /// whose node spans can legitimately contain comment bytes nothing else
    /// prints: the `{@debug}` identifier emitter, where a JSDoc-cast entry's span
    /// (`(a)`) can hold an interior comment (`(a /* c */)`) that reaches no
    /// comment emitter.
    pub(crate) fn source_span_covering_comments_doc(&self, span: Span) -> DocId {
        #[cfg(feature = "comment_check")]
        tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);

        self.d().source_span(span, self.source)
    }

    /// The frozen slice for a node the freeze resolved, **plus the owned-comment claim it
    /// owes** — the Svelte twin of `tsv_ts`'s `build_frozen_node_doc`, and the emitter
    /// every value-head freeze in this printer goes through.
    ///
    /// A block comment glued before the frozen node is *owned* by it, so the slice owes
    /// the claim ([`Self::claim_owned_leading_comment`]). It sits outside the slice's own
    /// span, so widening the slice is not the fix; the claim is. Prettier keeps the
    /// comment right where the author glued it, before the frozen value, and so does this.
    pub(in crate::printer) fn build_frozen_node_doc(&self, span: Span) -> DocId {
        self.claim_owned_leading_comment(self.verbatim_source_doc(span), span.start)
    }

    /// Prepend the block comment **owned** by the node beginning at `start`, when one is
    /// bound there (glued on the token's own line, or — for a JSDoc cast — from the line
    /// above its `(`) — the claim any builder owes that assembles a node's doc itself
    /// instead of routing through `tsv_ts`'s comment-aware expression builder (which
    /// makes the claim via `prepend_owned_leading_comment`).
    ///
    /// Two such builders exist in this printer: the freeze above, and the binding-pattern
    /// printer's object/array delimiter builders (`build_object_braces` /
    /// `build_array_brackets`, reached from [`Printer::build_pattern_doc`]). Both are
    /// docs/comments.md hazard 1 — the comment rides *inside* the doc these replace, and
    /// every gap emitter around it skips owned comments (the to-emit axis), so unless the
    /// builder prints it nothing does.
    ///
    /// ⚠️ The claim belongs to the node whose doc is being **assembled here**, never to a
    /// whole dispatch: two nested nodes can begin at the same offset (an
    /// `AssignmentPattern` and its `left`, a paren-less arrow and its parameter), and a
    /// claim made at both is a double print.
    ///
    /// ⚠️ **The separator is the author's, not a constant** — the same rule, and the same
    /// reason, as `tsv_ts`'s `prepend_owned_leading_comment_at`. Ownership's two producers
    /// bind at two different glues, so a claim that always writes a space is right only for
    /// the glued one: a JSDoc cast owns its comment from the line **above** its `(`, and
    /// pulling that comment up onto the `(`'s line is a relocation the unfrozen path does
    /// not make (`{@const a =⏎/** @type {T} */⏎(b)}` keeps the break, and so does prettier).
    /// Reading the newline off the source keeps the two claims answering with one rule.
    pub(in crate::printer) fn claim_owned_leading_comment(&self, doc: DocId, start: u32) -> DocId {
        if !self.has_owned_comments {
            return doc;
        }
        let Some(comment) = tsv_lang::owned_leading_comment_at(self.source, self.comments, start)
        else {
            return doc;
        };
        let d = self.d();
        let comment_doc = tsv_ts::build_comment_doc(d, comment, &self.ts_inputs());
        let separator = if tsv_lang::printing::has_newline_between_scan(
            self.source.as_bytes(),
            self.line_table(),
            comment.span.end,
            start,
        ) {
            d.hardline()
        } else {
            d.text(" ")
        };
        d.concat(&[comment_doc, separator, doc])
    }

    /// A braced head's **value stage**: the frozen slice when the head's gap resolved a
    /// freeze, else the comment-aware TS expression doc built under `embed`.
    ///
    /// Every braced value in this printer asks exactly this question — the tags
    /// (`{@html …}`, `{@render …}`, `{@const … = …}`), the block heads (`{#if …}`,
    /// `{#each …}`, an `{#each}` key), the attribute `={…}` values, the `{expr}` tag, and
    /// the braced attribute heads (`{...rest}`, `{@attach …}`) — after each has resolved
    /// its own freeze verdict ([`Self::honored_directive_in_gap`]) and its own
    /// [`EmbedContext`]. One spelling, because the frozen arm owes the owned-comment claim
    /// ([`Self::build_frozen_node_doc`], docs/comments.md hazard 1) and a site that
    /// re-spells the branch is a site that can forget it.
    ///
    /// What stays with the caller is what genuinely differs per head, and each difference
    /// is load-bearing, so the bodies around this call are deliberately NOT shared:
    ///
    /// - the **`EmbedContext`** — three distinct recipes. The braced heads (block heads
    ///   and prefixed tags alike) share one, [`Self::head_embed`]; `{@const}`'s init
    ///   inherits the host's Standalone mode so a root binary stays Grouped under the
    ///   assignment layout; an attribute value starts from [`EmbedContext::default`],
    ///   not the host's.
    /// - the **post-processing** — `remove_lines` for an inline block head,
    ///   [`Self::indent_own_line_head`] for a prefixed head, nothing for the rest. The
    ///   leading-line-comment continuation indent is NOT in this list: it is a property
    ///   of the *comment*, not of the head, so it is [`Self::head_layout`] and every
    ///   braced head asks it.
    ///
    /// A value whose leading comment is a **JSDoc cast** rides one more per-head verdict:
    /// `EmbedContext::jsdoc_cast_cannot_hang`, the "answers the break by rule and CANNOT
    /// hang" value-gap category. A hugging braced head has no operator line to end — the
    /// value starts right after `{#if ` — so the hardline an own-line cast comment earns
    /// everywhere else would strand the `(` at the head's own column (no fixed point where
    /// the head's group flattens, a column-0 value where it can't); the flag makes the cast
    /// reflow to the glued one-line form instead (`{#if /** @type {A} */ (aa)}`). Every
    /// hugging head sets it in its embed recipe ([`Self::head_embed`], the block-head inline
    /// arm, the braced attribute heads, the expression tag); the two heads that CAN give the
    /// comment a real line keep it unset — `{@const}` hangs off its `=`, a directive value
    /// block-wraps. Prettier is no oracle here (it drops the cast comment outright at every
    /// one of these heads); see
    /// docs/conformance_prettier_svelte.md §Svelte: Own-line JSDoc cast at a braced head.
    ///
    /// ⚠️ **Hanging it instead is measured WRONG, not merely incomplete** — answering
    /// [`Self::head_layout`] a hanging arm for the shape indents the value but
    /// leaves the comment glued to `{#if ` (it is *owned*, riding inside the value's doc), so
    /// the output still reads as mid-line and still collapses — and it drags the heads whose
    /// group can't flatten into the same non-convergence. Measured on branch, reverted.
    pub(in crate::printer) fn build_head_value_doc(
        &self,
        expr: &Expression<'_>,
        frozen: bool,
        embed: &EmbedContext,
    ) -> DocId {
        if frozen {
            self.build_frozen_node_doc(expr.span())
        } else {
            tsv_ts::build_expression_doc(self.d(), expr, &self.ts_inputs(), *embed)
        }
    }

    /// The [`EmbedContext`] every **braced head** builds its value under — a block head
    /// (`{#if …}`, `{#each …}`, an `{#each}` key) and a prefixed tag (`{@html …}`,
    /// `{@render …}`) alike. `opening_offset` is the width of the text before the
    /// expression, derived from the emitted opening literal so the estimate and the text
    /// cannot drift.
    ///
    /// `mode` is the load-bearing field: the expression-ROOT entry
    /// (`build_root_expression_doc`) reads `is_embedded()` to pick ContinuationIndent over
    /// Grouped style for a root binary.
    ///
    /// `first_line_offset` is a width estimate that reaches **nothing** on this path — it is
    /// read only by `tsv_ts`'s own render entry (`write_arena_doc`, which a Svelte-embedded
    /// expression never takes: this printer builds the doc and renders it with its OWN embed)
    /// and by the renderer's `effective_suffix_width`, gated on a `suffix_width` that is 0
    /// here. Perturbing it changes no byte of any fixture or of a 9k-file real corpus. It is
    /// computed anyway so the two hosts cannot read as deliberately different — which is the
    /// whole reason this recipe is one function rather than a copy each.
    ///
    /// `jsdoc_cast_cannot_hang` is the recipe's second load-bearing field: a braced head's
    /// value hugs its prefix, so a leading cast's own-line hardline has no matching hang and
    /// must reflow (see [`Self::build_head_value_doc`]).
    ///
    /// ⚠️ `EmbedContext::root_sequence_indents` is deliberately **not** part of the recipe:
    /// the two heads have different oracles for a root sequence's wrap. Prettier
    /// width-wraps `{@html (a,⏎b)}` flush, so a prefixed tag must keep that shape; it never
    /// width-wraps a block head at all, so the block head owns its geometry and sets the
    /// field itself ([`Self::build_expression_doc_for_block`]). Everything the two heads do
    /// share stays here.
    pub(in crate::printer) fn head_embed(&self, opening_offset: usize) -> EmbedContext {
        EmbedContext {
            first_line_offset: TAB_WIDTH + opening_offset,
            mode: LayoutMode::Embedded,
            jsdoc_cast_cannot_hang: true,
            ..self.embed
        }
    }

    /// Resolve a braced head's [`HeadLayout`] from its `head`→value gap — the ONE spelling of
    /// that question, so the family cannot answer it three ways again.
    ///
    /// A **`//` in the gap** is what takes the head off `Inline`: it runs to end of line, so
    /// the value cannot stay on the head's line. Which hanging arm it takes is then the
    /// author's, read off the run's FIRST comment: written after the opening literal it keeps
    /// that line (`HangsAfterOpen`), written on its own line it keeps *that*
    /// (`OpensOwnLine`). The comment leads the head's VALUE, and own-line-ness is authoring
    /// signal for a leading position (conformance_prettier.md §Comment Position Philosophy),
    /// so collapsing the two would relocate one of them. Prettier collapses both onto the
    /// head's line, at every braced head.
    ///
    /// A **freeze** short-circuits to `OpensOwnLine`: an honored directive flush against the
    /// prefix is inert under the placement floor, so the break is what makes the freeze
    /// survive a second pass, not a nicety. It is the same shape the own-line authoring
    /// reaches on its own — the freeze is a special case of it, not a rule beside it.
    ///
    /// A leading **block** comment is deliberately excluded, multi-line or not. It ends with a
    /// space, never a hardline: a single-line one doesn't break the head at all, and a
    /// multi-line one's newlines live *inside* its verbatim source span, which renders with no
    /// context indent by design (the interior stays as authored), so its continuation is the
    /// comment's own line and there is nothing to indent. A gap holding only blocks is
    /// `Inline`.
    pub(in crate::printer) fn head_layout(
        &self,
        gap_start: u32,
        value_start: u32,
        frozen: bool,
    ) -> HeadLayout {
        if frozen {
            return HeadLayout::OpensOwnLine;
        }
        let mut run = comments_to_emit_in_range(self.comments, gap_start, value_start).peekable();
        let Some(first) = run.peek() else {
            return HeadLayout::Inline;
        };
        let first_own_line =
            tsv_lang::source_scan::has_newline_before_position(self.source, first.span.start);
        if !run.any(|c| !c.is_block) {
            return HeadLayout::Inline;
        }
        if first_own_line {
            HeadLayout::OpensOwnLine
        } else {
            HeadLayout::HangsAfterOpen
        }
    }

    /// The **clarity parens** an assignment used as a value needs (`{@html (a = b)}`,
    /// `prop={(a = b)}`, `{...(a = b)}`) — `value_doc` wrapped for an assignment, returned
    /// unchanged for every other expression, which prints whatever parens it needs itself.
    ///
    /// They are the *printer's* parens, not the author's, which is why they are applied here
    /// rather than inside [`Self::build_head_value_doc`]: a frozen slice keeps its interior
    /// exactly as authored and the parens are **re-synthesized around it**, the same way the
    /// prefix keyword and the closing `}` stay outside the freeze
    /// (docs/conformance_prettier_ignore.md §Format-ignore directive). Skipping them on the
    /// frozen arm doesn't preserve more of the
    /// author's text — it *deletes* the parens they wrote. They must also land inside the
    /// head's own break: applied after the head is assembled instead, a frozen `{@html`
    /// emitted `{@html(`.
    ///
    /// Every braced value position owes them **except `{@const}`'s initializer**, where the
    /// paren is fully redundant and prettier drops it (`{@const a = (b = c)}` →
    /// `{@const a = b = c}`, though the same tool keeps it for a `<script>`'s
    /// `const a = (b = c)`) — that site calls `build_head_value_doc` alone.
    pub(in crate::printer) fn wrap_value_clarity_parens(
        &self,
        expr: &Expression<'_>,
        value_doc: DocId,
    ) -> DocId {
        if matches!(expr, Expression::AssignmentExpression(_)) {
            self.d().parens(value_doc)
        } else {
            value_doc
        }
    }

    /// An own-line head's content, broken onto its own indented lines: a hardline, then the
    /// gap's comment run and the value one level in.
    ///
    /// The shape [`HeadLayout::OpensOwnLine`] selects, for both of its causes. A
    /// **frozen** head needs it because a directive flush against the prefix is inert under
    /// tsv's floor and the freeze would be gone on the second pass; an **own-line leading
    /// run** needs it because pulling the comment up onto the prefix's line relocates it. The
    /// unprefixed `{…}` values reach the same shape through `wrap_in_block_structure`, and the
    /// hardline here is also what breaks the enclosing head group — a
    /// [`Self::verbatim_source_doc`] slice is deliberately opaque to `will_break`, so a frozen
    /// one cannot break anything by itself.
    ///
    /// The caller supplies the prefix via [`Self::head_open_doc`] and its own closing token
    /// after the break.
    pub(in crate::printer) fn indent_own_line_head(&self, content: DocId) -> DocId {
        let d = self.d();
        d.indent_hardline(content)
    }

    /// A braced head's assembled content wearing whatever **indents** it — the one ladder
    /// every head builder's final shape comes out of.
    ///
    /// [`HeadLayout::OpensOwnLine`] takes [`Self::indent_own_line_head`], which also supplies
    /// the hardline that opens the line; [`HeadLayout::HangsAfterOpen`] takes the plain
    /// indent, the run's first comment having already ended the opening literal's line;
    /// [`HeadLayout::Inline`] stays where it is. The layout is resolved inside the builder
    /// that knows it rather than at the assembler, so every caller assembles the same shape
    /// ([`Self::build_prefixed_head_doc`]).
    ///
    /// ⚠️ [`HeadLayout::indents_content`] is also exactly what the builder owes
    /// [`Self::trailing_comment_docs`] as `closer_owns_break` — the indent applied here is
    /// literally the thing that question asks about. One ladder for both so the applied
    /// indent and the closer's break cannot disagree.
    pub(in crate::printer) fn indent_head_content(
        &self,
        content: DocId,
        layout: HeadLayout,
    ) -> DocId {
        match layout {
            HeadLayout::OpensOwnLine => self.indent_own_line_head(content),
            HeadLayout::HangsAfterOpen => self.d().indent(content),
            HeadLayout::Inline => content,
        }
    }

    /// The **tail every braced head shares**: the leading run over the `head`→value gap, the
    /// value, the trailing run up to the closer, and the indent ladder — assembled from ONE
    /// `hangs` verdict, taken above the trailing run it governs and fed to both
    /// [`Self::trailing_comment_docs`]'s `closer_owns_break` and [`Self::indent_head_content`].
    ///
    /// The caller owns everything ABOVE this: the freeze verdict, the embed its value was
    /// measured under, the clarity parens, and any per-site post-processing (`remove_lines`
    /// for an inline block head). What it must not own is the pairing here — three builders
    /// each spelled this tail out and answered the hang three different ways, which is the
    /// whole reason the seam exists.
    ///
    /// `gap_start` is just past the head's opening literal, `content_end` just before the
    /// closer. The other braced heads stay out by construction, each for a stated reason: the
    /// unprefixed `{…}` ([`Self::build_expression_content_with_comments`]) cannot apply its own
    /// indent, since its caller picks the layout; a `{@const}` init takes `closer_owns_break`
    /// from the assignment layout rather than from a hang; and `{@debug}` interleaves its
    /// comments with an identifier list on the in-source axis.
    pub(in crate::printer) fn assemble_head_expr(
        &self,
        value_doc: DocId,
        gap_start: u32,
        value: Span,
        content_end: u32,
        frozen: bool,
    ) -> HeadExpr {
        let layout = self.head_layout(gap_start, value.start, frozen);
        let leading_docs = self.leading_comment_docs(gap_start, value.start);
        let (trailing_docs, ends_with_line_comment) =
            self.trailing_comment_docs(value.end, content_end, layout.indents_content());
        let body = self.concat_with_surrounding_comments(leading_docs, value_doc, trailing_docs);
        HeadExpr {
            doc: self.indent_head_content(body, layout),
            layout,
            ends_with_line_comment,
            owes_continuation_indent: false,
        }
    }

    /// The opening literal of a prefixed head, as a doc. A [`HeadLayout::OpensOwnLine`] head
    /// drops the literal's trailing space: its content
    /// begins with its own hardline, so the space would be trailing whitespace on the
    /// prefix's line.
    pub(in crate::printer) fn head_open_doc(
        &self,
        open: &'static str,
        opens_own_line: bool,
    ) -> DocId {
        self.d().text(if opens_own_line {
            open.trim_end()
        } else {
            open
        })
    }

    /// A whole prefixed head — the opening literal, the content, the closing token — for
    /// every head that owns its closing token directly: the tags (`{@html …}`,
    /// `{@render …}`, `{@debug …}`), the braced attribute heads (`{...}`, `{@attach}`), and
    /// an `{#each}` key's parens. **The single assembler for that shape**, so the frozen
    /// head's three coupled adjustments — space-less prefix, own-line content, dangling
    /// closer — cannot be applied at one site and forgotten at the next.
    ///
    /// The block heads are the deliberate exception: they close with their own tail (an
    /// `{#each}` ends `as item}`), so they take [`Self::head_open_doc`] and let their
    /// existing dangle supply the break. Both paths render the same content shape.
    ///
    /// `head.doc` is the content in its final shape — already through
    /// [`Self::indent_own_line_head`] when the content opens on its own line (see [`HeadExpr`]).
    pub(in crate::printer) fn build_prefixed_head_doc(
        &self,
        open: &'static str,
        head: HeadExpr,
        close: &'static str,
    ) -> DocId {
        let d = self.d();
        if !head.layout.indents_content() {
            // Nothing indents this content, so a trailing line comment's own `hardline` is
            // already the break the `}` needs, on the right column.
            return d.concat(&[d.text(open), head.doc, d.text(close)]);
        }
        // Whatever indented the content, the closer drops to the head's own column — the
        // question is [`HeadLayout::indents_content`], never which arm indented it, so the two
        // hanging authorings of one head differ in the comment's line and in nothing else.
        //
        // A run-final line comment already broke the line, dedented out of that indent
        // (`build_trailing_js_comment_doc`), so the closer reuses that break and lands where
        // it also lands with no trailing comment at all. A second break would render as a
        // blank line above it.
        let open_doc = self.head_open_doc(open, head.layout.opens_own_line());
        if head.ends_with_line_comment {
            return d.concat(&[open_doc, head.doc, d.text(close)]);
        }
        d.concat(&[open_doc, head.doc, d.hardline(), d.text(close)])
    }

    /// A head's content for an assembler that **hugs** its delimiters — the one seam that
    /// settles [`HeadExpr::owes_continuation_indent`], so a new hugging arm cannot silently
    /// leave a hung value flush. The block-wrapping arms take `head.doc` directly: their
    /// `indent(…)` already is the continuation indent, and a second would double it.
    pub(in crate::printer) fn hug_head_content(&self, head: HeadExpr) -> DocId {
        if head.owes_continuation_indent {
            self.d().indent(head.doc)
        } else {
            head.doc
        }
    }

    /// Get the source code
    pub(crate) fn source(&self) -> &str {
        self.source
    }

    /// The document's line table — what every embedded TypeScript island reads its line
    /// questions from (the table is the whole document's; spans are absolute).
    pub(crate) fn line_table(&self) -> tsv_lang::printing::LineTable<'_> {
        self.line_breaks.table()
    }

    /// Standard [`tsv_ts::PrinterInputs`] for embedding TypeScript: this
    /// document's source, comments, and line breaks. Call sites
    /// needing empty comments override via
    /// `PrinterInputs { comments: &[], ..self.ts_inputs() }`.
    pub(crate) fn ts_inputs(&self) -> tsv_ts::PrinterInputs<'_> {
        tsv_ts::PrinterInputs {
            source: self.source,
            comments: self.comments,
            line_table: self.line_table(),
            // The document-level owned-comment flag, computed once at construction
            // (never here — this is called per `{expr}`; see the field's doc).
            has_owned_comments: self.has_owned_comments,
            // Likewise the document-level format-ignore flag (computed once at construction).
            has_format_ignore: self.has_format_ignore,
        }
    }

    /// Write indentation based on current indent level
    pub(crate) fn write_indent(&mut self) {
        tsv_lang::write_indent(&mut self.buffer, self.indent_level, INDENT);
    }

    /// Get the formatted output
    ///
    /// Simply extracts the buffer. Whitespace stripping is handled by the doc rendering layer:
    /// - Normal elements: rendered with `print_doc_with_indent_resolved()` which strips
    /// - Whitespace-sensitive elements: rendered with `print_doc_with_indent_resolved_preserve_whitespace()` which preserves
    pub(crate) fn into_string(self) -> String {
        // Park the line-break scratch back on the arena for the next format
        // (capacity retained, filled or not; see `with_embed`).
        self.arena
            .park_line_breaks_scratch(self.line_breaks.into_scratch());
        self.buffer.into_string()
    }

    /// Render a DocId immediately at current buffer position
    ///
    /// This is the foundation for doc-first formatting. Instead of using
    /// imperative printing, callers build a Doc and render it in one step.
    ///
    /// The doc is rendered starting at the current column position with
    /// the current indent level, so it seamlessly integrates with any
    /// preceding output.
    ///
    /// Always uses the preserve-whitespace variant because the doc tree may contain
    /// whitespace-sensitive elements (`<pre>`, `<textarea>`) whose trailing whitespace
    /// must be preserved. Normal elements have trailing whitespace stripped during
    /// doc building, not rendering.
    pub(crate) fn render_doc_immediate(&mut self, d: DocId) {
        let col = self.buffer.current_column(TAB_WIDTH);
        // Render into the arena-parked scratch: one warm buffer across the
        // document's root nodes instead of an alloc/free per node.
        let mut output = self.arena.take_render_scratch();
        // Pass the document source: the doc tree's verbatim leaves — this
        // printer's own markup text / comment slices plus any embedded
        // `tsv_ts` docs — are `DocText::SourceSpan` (host-absolute spans).
        tsv_lang::doc::arena_print_doc_with_indent_resolved_preserve_whitespace_into(
            self.arena,
            d,
            &self.embed,
            col,
            self.indent_level,
            self.source,
            &mut output,
        );
        self.write(&output);
        self.arena.park_render_scratch(output);
    }
}

impl<'a> Printer<'a> {
    /// Build a DocId for a TS expression (with comments) in our arena.
    ///
    /// Uses the standard parameters: self.comments, self.embed, self.line_breaks — i.e. the
    /// unfrozen arm of [`Self::build_head_value_doc`] under the host's own embed, spelled
    /// once there. For calls that need a custom embed context or empty comments, use the
    /// tsv_ts functions directly.
    pub(crate) fn build_ts_expression_doc(&self, expr: &Expression<'_>) -> DocId {
        self.build_head_value_doc(expr, false, &self.embed)
    }

    /// [`Self::build_ts_expression_doc`] for a **braced-head** value: the host's embed plus
    /// the leading-cast reflow every hugging braced head owes (`jsdoc_cast_cannot_hang` —
    /// see [`Self::build_head_value_doc`]). The `{#each}` key's fallback arm and the
    /// pattern printer's computed key use it; the main key arm reaches the same flag
    /// through [`Self::build_expression_doc_for_block`].
    pub(crate) fn build_ts_expression_doc_cannot_hang(&self, expr: &Expression<'_>) -> DocId {
        self.build_head_value_doc(expr, false, &self.cannot_hang_embed())
    }

    /// The host's embed plus `jsdoc_cast_cannot_hang` — the recipe for a hugging braced
    /// head measured where it sits (the block-head inline arm, a prefixed attribute
    /// head); the heads that re-base their measurement carry the flag in their own
    /// recipes instead ([`Self::head_embed`], the unprefixed value's default-based embed).
    pub(in crate::printer) fn cannot_hang_embed(&self) -> EmbedContext {
        EmbedContext {
            jsdoc_cast_cannot_hang: true,
            ..self.embed
        }
    }
}

/// Format a Svelte AST back to source code
pub(crate) fn format_svelte(root: &internal::Root<'_>, source: &str) -> String {
    let arena = DocArena::for_source(source);
    format_svelte_in(root, source, &arena)
}

/// Format a Svelte AST into a caller-provided doc arena (the reuse path).
pub(crate) fn format_svelte_in(
    root: &internal::Root<'_>,
    source: &str,
    arena: &DocArena,
) -> String {
    // The print-once comment ledger's expectation for this document (diagnostic; see
    // `tsv_lang::comment_ledger`). `Root.comments` is the `<script>` + template-expression
    // JS comments; the `<style>` island registers its own through `tsv_css`. The template's
    // `<!-- -->` (`FragmentNode::Comment`) comments are AST nodes rather than detached, so
    // they register by span through a recursive fragment walk — hoisted section comments
    // included, since they still live in `Root.fragment.nodes` (see `print_root`).
    #[cfg(feature = "comment_check")]
    {
        tsv_lang::comment_ledger::register_parsed(source, &root.comments);
        let mut html_comment_spans = Vec::new();
        collect_html_comment_spans(&root.fragment, &mut html_comment_spans);
        tsv_lang::comment_ledger::register_parsed_spans(source, html_comment_spans);
    }

    let mut printer = Printer::new(arena, source, &root.comments);
    printer.print_root(root);
    printer.into_string()
}

/// Collect the spans of every `<!-- -->` (`FragmentNode::Comment`) in a fragment, recursing
/// into every nested fragment (elements, special elements, and the `{#if}` / `{#each}` /
/// `{#await}` / `{#key}` / `{#snippet}` block bodies). The print-once comment ledger reads
/// only the span, so no `HtmlComment` need be manufactured into a `Comment`.
#[cfg(feature = "comment_check")]
fn collect_html_comment_spans(fragment: &internal::Fragment<'_>, out: &mut Vec<Span>) {
    for node in fragment.nodes {
        match node {
            FragmentNode::Comment(comment) => out.push(comment.span),
            FragmentNode::Element(el) => collect_html_comment_spans(&el.fragment, out),
            FragmentNode::SpecialElement(el) => collect_html_comment_spans(&el.fragment, out),
            FragmentNode::IfBlock(block) => {
                collect_html_comment_spans(&block.consequent, out);
                if let Some(alternate) = &block.alternate {
                    collect_html_comment_spans(alternate, out);
                }
            }
            FragmentNode::EachBlock(block) => {
                collect_html_comment_spans(&block.body, out);
                if let Some(fallback) = &block.fallback {
                    collect_html_comment_spans(fallback, out);
                }
            }
            FragmentNode::AwaitBlock(block) => {
                if let Some(pending) = &block.pending {
                    collect_html_comment_spans(pending, out);
                }
                if let Some(then) = &block.then {
                    collect_html_comment_spans(then, out);
                }
                if let Some(catch) = &block.catch {
                    collect_html_comment_spans(catch, out);
                }
            }
            FragmentNode::KeyBlock(block) => collect_html_comment_spans(&block.fragment, out),
            FragmentNode::SnippetBlock(block) => collect_html_comment_spans(&block.body, out),
            _ => {}
        }
    }
}

impl<'a> Printer<'a> {
    /// Whether the gap `[start, end)` holds a directive that honors — the one freeze
    /// question the Svelte printer asks, in both of the shapes it has:
    ///
    /// - the **value head**: the `{`→value gap of a braced expression (a directive value,
    ///   an expression tag), the Svelte instance of the delimiter-owned head that `tsv_ts`
    ///   spells `Printer::value_head_frozen_span`. Freezes the whole value; the `}` that
    ///   closes it stays parent-owned.
    /// - **Rule A**: a function-binding sequence's inter-operand gap. Freezes the
    ///   FOLLOWING operand; the `,` stays parent-owned. The same rule `tsv_ts`'s sequence
    ///   printer applies — the bind value has its own operand loop only because Svelte
    ///   prints the pair bare.
    ///
    /// Both callers slice their own span from the `bool`, so unlike `tsv_ts` there is
    /// nothing for a per-rule name to carry; each call site names its rule in a comment
    /// instead of behind a hollow wrapper.
    ///
    /// It also decides the gap's **layout**, which is not a second question: an honored
    /// directive takes the broken block form, so the emitter never pulls it flush against
    /// the `{`, where it would be inert and the freeze would be lost on the second pass
    /// (the `{…}` instance of the declaration-header rule; see
    /// docs/conformance_prettier_ignore.md §Format-ignore directive).
    ///
    /// **In-source axis** — a directive is never owned, so the axes coincide, but naming
    /// the physical one keeps directive recognition a single deliberate question (as
    /// `tsv_ts`'s `member_gap_frozen` does). Opens on the document-level flag: every braced
    /// value in every component asks it.
    pub(in crate::printer) fn honored_directive_in_gap(&self, start: u32, end: u32) -> bool {
        self.has_format_ignore
            && comments_in_source_range(self.comments, start, end)
                .any(|c| self.is_honored_directive(c))
    }

    /// Whether comment `c` is a format-ignore directive that HONORS — `tsv_ts`'s
    /// `Printer::is_honored_directive` against this document. The freeze tests ask it
    /// through [`Self::honored_directive_in_gap`]; an emitter asks it directly when a
    /// comment's own layout depends on the answer, because an honored directive must keep
    /// the line the author gave it — gluing the following construct onto that line makes the
    /// placement inert, and the freeze would be lost on the next pass.
    ///
    /// Carries the document-level `has_format_ignore` gate, so a directive-free component
    /// (≈ every component) pays one predicted branch instead of the content compare.
    pub(in crate::printer) fn is_honored_directive(&self, c: &Comment) -> bool {
        self.has_format_ignore && is_honored_format_ignore(self.source, c)
    }

    /// Check if the last non-whitespace fragment node before `target_start` is
    /// a `<!-- format-ignore -->` (or `prettier-ignore`) comment.
    fn has_format_ignore_before(
        &self,
        fragment: &internal::Fragment<'_>,
        target_start: u32,
    ) -> bool {
        let mut last_comment = None;
        for node in fragment.nodes {
            let node_end = node.span().end;
            if node_end > target_start {
                break;
            }
            match node {
                FragmentNode::Comment(comment) => {
                    last_comment = Some(comment);
                }
                FragmentNode::Text(text) if text.is_collapsible_ws_only => {
                    // Skip whitespace text nodes
                }
                _ => {
                    // Non-comment, non-whitespace node resets
                    last_comment = None;
                }
            }
        }
        last_comment.is_some_and(|c| is_format_ignore_directive(c.content(self.source)))
    }

    /// Whether the fragment node at `idx` sits **inside a frozen range** — the nearest range
    /// marker before it opens one, and a closing marker follows.
    ///
    /// Mirrors the freeze condition in [`Self::print_root_fragment`] (a `…-start` counts only
    /// once a matching `…-end` is found), so an *unclosed* `…-start` — which tsv prints as an
    /// ordinary comment, freezing nothing — does not pin everything after it.
    fn is_inside_ignore_range(&self, idx: usize, fragment: &internal::Fragment<'_>) -> bool {
        let opened = fragment.nodes[..idx].iter().rev().find_map(|n| match n {
            FragmentNode::Comment(c) => {
                range_marker(c, self.source).map(|m| m == RangeMarker::Start)
            }
            _ => None,
        });
        opened == Some(true)
            && fragment.nodes[idx..].iter().any(|n| {
                matches!(n, FragmentNode::Comment(c)
                    if range_marker(c, self.source) == Some(RangeMarker::End))
            })
    }

    /// Classify which section a fragment comment should travel with during
    /// canonical reordering. Each comment attaches to the nearest section
    /// that follows it in source order.
    ///
    /// Two things pin a comment to the template: a real node between it and every section (it
    /// leads that node, not a section), and a **format-ignore range marker** — see
    /// [`range_marker`]'s role in the scan below.
    fn classify_fragment_comment(
        &self,
        comment: &internal::HtmlComment,
        comment_idx: usize,
        root: &internal::Root<'_>,
    ) -> CommentSection {
        // format-ignore-start/end mark ranges within the template —
        // they must stay in the fragment so the range preservation logic sees them
        if range_marker(comment, self.source).is_some() {
            return CommentSection::Template;
        }

        // A comment INSIDE a frozen range is already emitted by the range's verbatim slice, so
        // hoisting it prints it twice. The forward scan below cannot see this on its own: when
        // the *section* is inside the range too, its start precedes the closing marker and wins
        // the nearest-start contest.
        if self.is_inside_ignore_range(comment_idx, &root.fragment) {
            return CommentSection::Template;
        }

        let comment_end = comment.span.end;
        let mut nearest: Option<(u32, CommentSection)> = None;

        // The first thing after the comment that pins it to the template: a real node (the
        // comment leads *it*), or a range marker.
        //
        // A marker is a BARRIER, not a skippable comment. Hoisting across one moves the comment
        // into or out of a frozen region — a reorder prettier does not make — and when the
        // comment sits INSIDE the range it is printed twice, since the fragment emits the whole
        // range verbatim while the section emits the comment again. Ordinary comments are still
        // skipped: a run of them all lead the same section.
        for node in root.fragment.nodes.iter().skip(comment_idx + 1) {
            let barrier = match node {
                FragmentNode::Text(t) if t.is_collapsible_ws_only => continue,
                FragmentNode::Comment(c) if range_marker(c, self.source).is_some() => c.span.start,
                FragmentNode::Comment(_) => continue,
                other => other.span().start,
            };
            if barrier >= comment_end {
                nearest = Some((barrier, CommentSection::Template));
            }
            break;
        }

        // Every root section competes on the same rule — the nearest start after the comment
        // wins. One loop over the shared [`root_sections`] table rather than four copies of
        // the comparison: the copies differ only in which field and which variant, which is
        // exactly the pair a copy-paste edit gets wrong silently.
        for (span, section) in root_sections(root) {
            let Some(start) = span.map(|s| s.start) else {
                continue;
            };
            if start >= comment_end && nearest.as_ref().is_none_or(|(p, _)| start < *p) {
                nearest = Some((start, section));
            }
        }

        nearest.map_or(CommentSection::Template, |(_, section)| section)
    }

    /// Print section-attached comments and preserve authorial blank lines.
    /// Returns true if any comments were printed.
    ///
    /// Both blank questions ask [`text::has_authored_blank_line`], the RUN scan, because neither
    /// gap is guaranteed whitespace-only: a `format-ignore` range marker classifies
    /// [`CommentSection::Template`] while [`Self::classify_fragment_comment`] skips *every*
    /// comment when it looks for the nearest real node, so a marker can sit between two section
    /// comments — or between the last one and its section. A newline *total* would read that
    /// marker's two bracketing newlines as an authored blank and invent one.
    ///
    /// TODO: the marker only lands in those gaps because a section comment is hoisted **past**
    /// the range boundary in the first place — a reorder prettier does not make, and one that
    /// double-prints a comment written INSIDE the range (the fragment emits the range verbatim
    /// while the section emits the comment again). Classifying a comment separated from its
    /// section by a range marker as `Template` would fix the reorder, the double-print, and
    /// make these gaps whitespace-only again; it needs its own fixture cycle.
    fn print_section_comments(
        &mut self,
        comment_indices: &[usize],
        fragment: &internal::Fragment<'_>,
        section_start: u32,
    ) -> bool {
        if comment_indices.is_empty() {
            return false;
        }
        let mut prev_end: Option<u32> = None;
        for &i in comment_indices {
            if let FragmentNode::Comment(comment) = &fragment.nodes[i] {
                // Preserve authorial blank line between consecutive comments
                if let Some(end) = prev_end {
                    let between = &self.source[end as usize..comment.span.start as usize];
                    if text::has_authored_blank_line(between) {
                        self.write("\n");
                    }
                }
                self.print_comment(comment);
                self.write("\n");
                prev_end = Some(comment.span.end);
            }
        }
        // Preserve authorial blank line between last comment and section
        if let Some(&last_idx) = comment_indices.last() {
            let last_end = fragment.nodes[last_idx].span().end;
            let between = &self.source[last_end as usize..section_start as usize];
            if text::has_authored_blank_line(between) {
                self.write("\n");
            }
        }
        true
    }

    /// Format a Svelte Root node
    ///
    /// Orchestrates formatting of the four main sections of a .svelte file:
    /// 1. Module script: `<script context="module">`
    /// 2. Instance script: `<script>`
    /// 3. Template: The HTML/Svelte template
    /// 4. Style: `<style>`
    ///
    /// Sections are ordered canonically and separated by blank lines.
    /// Comments travel with the section they immediately precede in source order.
    pub(crate) fn print_root(&mut self, root: &internal::Root<'_>) {
        // Classify fragment comments by the section they should travel with.
        let mut options_comments: Vec<usize> = Vec::new();
        let mut module_comments: Vec<usize> = Vec::new();
        let mut instance_comments: Vec<usize> = Vec::new();
        let mut style_comments: Vec<usize> = Vec::new();

        for (i, node) in root.fragment.nodes.iter().enumerate() {
            if let FragmentNode::Comment(comment) = node {
                match self.classify_fragment_comment(comment, i, root) {
                    CommentSection::Options => options_comments.push(i),
                    CommentSection::ModuleScript => module_comments.push(i),
                    CommentSection::InstanceScript => instance_comments.push(i),
                    CommentSection::Style => style_comments.push(i),
                    CommentSection::Template => {}
                }
            }
        }

        // Sections are lifted off the fragment and printed at canonical positions, but a
        // `format-ignore` range's verbatim slice is raw source and would re-emit any that sits
        // inside it — see `build_ignore_range_doc`.
        let hoisted: SmallVec<[Span; 4]> = root_sections(root)
            .into_iter()
            .filter_map(|(span, _)| span)
            .collect();

        // Non-template comments are skipped during fragment printing
        let mut printed_comment_indices: Vec<usize> = Vec::new();
        printed_comment_indices.extend(&options_comments);
        printed_comment_indices.extend(&module_comments);
        printed_comment_indices.extend(&instance_comments);
        printed_comment_indices.extend(&style_comments);

        let mut has_previous_section = false;

        // Format svelte:options (if present) - always first
        if let Some(options) = &root.options {
            self.print_section_comments(&options_comments, &root.fragment, options.span.start);
            self.print_svelte_options(options);
            has_previous_section = true;
        }

        // Format scripts (module then instance)
        for (script, comments) in [
            (root.module.as_ref(), &module_comments),
            (root.instance.as_ref(), &instance_comments),
        ] {
            if let Some(script) = script {
                if has_previous_section {
                    self.write("\n"); // Blank line between sections
                }
                self.print_section_comments(comments, &root.fragment, script.span.start);
                if self.has_format_ignore_before(&root.fragment, script.span.start) {
                    self.write_verbatim_span(script.span);
                    self.write("\n");
                } else {
                    self.print_script(script);
                }
                has_previous_section = true;
            }
        }

        // Format template fragment (if not empty)
        let has_content = root.fragment.nodes.iter().enumerate().any(|(i, node)| {
            if printed_comment_indices.contains(&i) {
                return false;
            }
            !matches!(node, FragmentNode::Text(text) if text.is_collapsible_ws_only)
        });

        if has_content {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.print_root_fragment(&root.fragment, &printed_comment_indices, &hoisted);
            self.write("\n"); // Template needs explicit newline
            has_previous_section = true;
        }

        // Format style (if present)
        if let Some(style) = &root.css {
            let ignore_style = self.has_format_ignore_before(&root.fragment, style.span.start);
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.print_section_comments(&style_comments, &root.fragment, style.span.start);
            if ignore_style {
                self.write_verbatim_span(style.span);
                self.write("\n");
            } else {
                self.print_style(style);
            }
        }
    }

    /// Format the `<svelte:options ... />` tag, always in the self-closing form.
    ///
    /// Hoisted out of the fragment and printed here at its canonical position, so it is the
    /// one tag head outside the element pipeline — but its attribute list is read by the
    /// ordinary `read_attribute` like any other, comments included, so it goes through the
    /// same [`Printer::push_attrs_with_comments`] the pipeline uses. Doc-based from there on,
    /// for width-aware wrapping.
    fn print_svelte_options(&mut self, options: &internal::SvelteOptions<'_>) {
        let d = self.d();
        // Built before the empty check, not after: the attribute list can be empty and still
        // carry comments (`<svelte:options /* c */ />`), and an early return keyed on
        // `attributes.is_empty()` deletes them — the comment-blind alternate arm, at the one
        // tag whose head is printed from here rather than from the element pipeline.
        let mut parts: DocBuf = DocBuf::with_capacity(options.attributes.len() * 2);
        self.push_attrs_with_comments(
            &mut parts,
            options.attributes,
            d.line(),
            AttrGaps {
                first_range_start: options.name_end,
                open_tag_end: options.open_tag_end,
                // Nothing here is synthesized: `<svelte:options>` has no `this` binding, so
                // every attribute in the window is in `attributes`.
                claimed: None,
            },
            false,
        );
        if parts.is_empty() {
            self.write("<svelte:options />\n");
            return;
        }

        let attrs = d.concat(&parts);
        // A comment's own break counts the same as an attribute value's — both are reasons
        // the head cannot stay on one line — so the question is asked of the assembled list.
        let has_multiline = d.will_break(attrs);
        let attr_indent = d.indent(attrs);
        let line = d.line();
        let inner = d.concat(&[d.text("<svelte:options"), attr_indent, line, d.text("/>")]);

        let group = if has_multiline {
            d.group_break(inner)
        } else {
            d.group(inner)
        };

        self.render_doc_immediate(group);
        self.write("\n");
    }

    /// Render the whole template fragment through the same doc-based content path the
    /// elements use — the root is **not special**.
    ///
    /// Prettier prints the root markup with the same `printChildren` as element children,
    /// just inside a force-broken `group([…, hardline])`. [`Printer::build_nodes_doc_multiline`]
    /// *is* that force-broken layout, so the root's sibling separation, blank-line handling,
    /// block hugging, and the inline-element `>`-fold all come from the shared builder. Three
    /// root-only concerns are handled around that call:
    ///
    /// - **Boundary trim (B1):** the template content is the span of `fragment.nodes` from the
    ///   first to the last node that isn't a section comment printed with its section
    ///   (`skip_indices`) and isn't leading/trailing Unicode-whitespace-only text. Prettier trims
    ///   the fragment boundary with a Unicode `trim()`, so a leading nbsp-only node (content
    ///   mid-template) is dropped here. Section comments are *usually* boundary-contiguous, but a
    ///   section sitting mid-template leaves one interior; those are dropped by the segmentation
    ///   loop below (they are already printed with their hoisted section).
    /// - **Single-line inline runs (B4):** a `{x}{#if c}…{/if}` run with no newline must
    ///   *inner-break* a long body, not drop it — the load-bearing discriminator the element
    ///   path does not apply. [`Self::mark_root_inline_run_blocks`] marks those blocks so the
    ///   shared builder builds them in inline context (see `root_inline_run_block_starts`).
    /// - **`format-ignore` ranges (B6):** `<!-- format-ignore-start -->` … `-end` is a
    ///   *root-only* directive (it does not activate inside an element), so the content is
    ///   split at top-level ranges: each range emits its source verbatim, the surrounding
    ///   segments go through the shared builder. (Single-node `format-ignore` is handled by
    ///   the shared builder itself.)
    fn print_root_fragment(
        &mut self,
        fragment: &internal::Fragment<'_>,
        skip_indices: &[usize],
        hoisted: &[Span],
    ) {
        // Effective template range: drop section comments (`skip_indices`) and collapsible-ws-only
        // boundary text. Both kinds only occur at the boundaries, so the kept content is a
        // contiguous slice.
        //
        // ⚠️ The trim is [`is_collapsible_ws_char`], NOT `str::trim` (the Unicode `White_Space`
        // property, which is wider). The root fragment is a fragment like any other, so it owes
        // the same rule every element boundary follows: the trim stops at content. `str::trim`
        // deleted a root-boundary NBSP — content the compiler keeps (`regex_not_whitespace` =
        // `/[^ \t\r\n]/` matches U+00A0, so the node is not whitespace-only and survives:
        // `\u{a0}<div>block</div>` compiles to `<!---->\u{a0}<div>…`). prettier deletes it too,
        // so only the compiler oracle sees it — `svelte/elements/root_leading_nbsp_prettier_divergence`.
        let source = self.source;
        let skippable = |i: usize, n: &FragmentNode<'_>| {
            skip_indices.contains(&i)
                || matches!(n, FragmentNode::Text(t) if t.raw(source).trim_matches(is_collapsible_ws_char).is_empty())
        };
        let Some(start) = fragment
            .nodes
            .iter()
            .enumerate()
            .position(|(i, n)| !skippable(i, n))
        else {
            return;
        };
        // `rposition` finds at least `start`, so the fallback never triggers (panic-free).
        let end = fragment
            .nodes
            .iter()
            .enumerate()
            .rposition(|(i, n)| !skippable(i, n))
            .unwrap_or(start);
        let nodes = &fragment.nodes[start..=end];

        // Mark single-line-run control-flow blocks so the shared builder inner-breaks them (B4).
        self.mark_root_inline_run_blocks(nodes);

        // Split at `format-ignore` ranges and at interior section comments (both rare); most
        // templates are one segment.
        let mut out: DocBuf = DocBuf::new();
        let mut seg_start = 0;
        let mut i = 0;
        while i < nodes.len() {
            // A comment printed with its (hoisted) section — `skip_indices` indexes the full
            // `fragment.nodes`, so offset by `start`. These are *usually* boundary-contiguous
            // (trimmed away by `start`/`end`), but a `<script>`/`<style>`/`<svelte:options>`
            // sitting **mid-template** (template content on both sides) leaves its comment
            // interior to the slice. Drop it here so the shared builder doesn't re-emit it as
            // template content (it is already printed with its section). The gap is re-bridged
            // with the same boundary-aware separator the `format-ignore` range path uses.
            if skip_indices.contains(&(start + i)) {
                if nodes[seg_start..i]
                    .iter()
                    .any(|n| !n.is_whitespace_only_text())
                {
                    out.push(self.build_nodes_doc_multiline(&nodes[seg_start..i]));
                    if let Some(sep) = self.range_trailing_separator(nodes, i) {
                        out.push(sep);
                    }
                }
                seg_start = i + 1;
                i += 1;
                continue;
            }
            let is_range_start = matches!(
                &nodes[i],
                FragmentNode::Comment(c) if range_marker(c, source) == Some(RangeMarker::Start)
            );
            if is_range_start
                && let Some(range_end) = (i + 1..nodes.len()).find(|&j| {
                    matches!(&nodes[j],
                        FragmentNode::Comment(c) if range_marker(c, source) == Some(RangeMarker::End))
                })
            {
                // Segment up to and including the start comment (it prints normally).
                out.push(self.build_nodes_doc_multiline(&nodes[seg_start..=i]));
                // Verbatim source from just after the start comment through the end
                // comment — emit the slice as a span, no allocation.
                let raw_start = nodes[i].span().end;
                let raw_end = nodes[range_end].span().end;
                out.push(self.build_ignore_range_doc(Span::new(raw_start, raw_end), hoisted));
                // The whitespace after the end comment is trimmed by the next segment's
                // boundary, so re-emit it as the separator before that segment.
                if let Some(sep) = self.range_trailing_separator(nodes, range_end) {
                    out.push(sep);
                }
                seg_start = range_end + 1;
                i = range_end + 1;
                continue;
            }
            i += 1;
        }
        if seg_start < nodes.len() {
            out.push(self.build_nodes_doc_multiline(&nodes[seg_start..]));
        }

        if !out.is_empty() {
            let doc = self.d().concat(&out);
            self.render_doc_immediate(doc);
        }
    }

    /// The verbatim doc for a `format-ignore` range slice, with every **hoisted section** cut out.
    ///
    /// A `<script>` / `<style>` / `<svelte:options>` written inside the range is not a fragment
    /// node — it is lifted onto [`internal::Root`] and printed at its canonical position — but
    /// this slice is *raw source* between the two markers, which still holds its bytes. Emitting
    /// them verbatim prints the section **twice**, and the result is not a valid component:
    /// tsv's own parser rejects it (`Duplicate instance script found` / `Duplicate style tag
    /// found` / `Duplicate <svelte:options> found`). So the slice is emitted as the pieces
    /// *between* the hoisted spans. Prettier drops the section from its own ignored range the
    /// same way; the range freezes template formatting, it does not pin a section's position.
    ///
    /// Each cut takes the section's span **plus the whitespace run immediately before it**, which
    /// is what closes the line rather than leaving a blank one behind: source
    /// `</div>⏎⏎<script>…</script>⏎⏎<p>` has two runs around the section and must end at
    /// `</div>⏎⏎<p>`, so exactly one of them goes with it. Bounded by the slice start, so a
    /// section glued to the start marker cuts nothing extra.
    fn build_ignore_range_doc(&self, span: Span, hoisted: &[Span]) -> DocId {
        let mut cuts: SmallVec<[Span; 4]> = hoisted
            .iter()
            .copied()
            .filter(|h| span.contains(*h))
            .collect();
        if cuts.is_empty() {
            return self.verbatim_source_doc(span);
        }
        // `hoisted` is in canonical print order (options, module, instance, style), which is not
        // source order — the pieces must be emitted in source order.
        cuts.sort_unstable_by_key(|s| s.start);

        let mut parts: DocBuf = DocBuf::new();
        let mut cursor = span.start;
        for cut in cuts {
            let piece_end = self.trim_trailing_ws_back_to(cursor, cut.start);
            if piece_end > cursor {
                parts.push(self.verbatim_source_doc(Span::new(cursor, piece_end)));
            }
            cursor = cut.end;
        }
        if cursor < span.end {
            parts.push(self.verbatim_source_doc(Span::new(cursor, span.end)));
        }
        self.d().concat(&parts)
    }

    /// Walk `to` back over collapsible whitespace, stopping at `floor`.
    ///
    /// The class is [`internal::is_collapsible_ws`] — the byte spelling of
    /// [`is_collapsible_ws_char`], the same set every other boundary in this printer trims;
    /// a form feed is content, so it stops the walk. A byte walk is exact here because the
    /// whole class is ASCII, and an ASCII byte value in UTF-8 is always a standalone char.
    fn trim_trailing_ws_back_to(&self, floor: u32, to: u32) -> u32 {
        let bytes = self.source.as_bytes();
        let mut end = to;
        while end > floor && internal::is_collapsible_ws(bytes[end as usize - 1]) {
            end -= 1;
        }
        end
    }

    /// Mark control-flow blocks in **single-line** root inline runs (`{x}{#if c}…{/if}` with no
    /// newline) so [`Printer::build_nodes_doc_multiline`] builds them in inline context — see
    /// [`Printer::root_inline_run_block_starts`]. A *multi-line* run (its content text spans
    /// source lines) keeps the multiline layout, so its blocks are left unmarked. Span-keyed,
    /// so only root-level run blocks are affected.
    fn mark_root_inline_run_blocks(&self, nodes: &[FragmentNode<'_>]) {
        let mut marks = self.root_inline_run_block_starts.borrow_mut();
        marks.clear();
        let mut i = 0;
        while i < nodes.len() {
            if let Some(run_end) = self.detect_root_inline_run(nodes, i) {
                let run = &nodes[i..=run_end];
                let multiline = run
                    .iter()
                    .any(|n| matches!(n, FragmentNode::Text(t) if t.has_newline()));
                if !multiline {
                    for n in run {
                        if nodes::is_control_flow_block(n) {
                            marks.insert(n.span().start);
                        }
                    }
                }
                i = run_end + 1;
            } else {
                i += 1;
            }
        }
    }

    /// The separator to emit after a `format-ignore` range, before the next segment: the
    /// whitespace immediately following the end comment (which the next segment's boundary
    /// trim would otherwise drop). A blank line → `literalline` (the un-indented blank) +
    /// `hardline`; a single newline / adjacency → `hardline`. `None` when nothing follows.
    ///
    /// "Immediately following" is the whole rule, so the blank is read off the next node's
    /// **leading** whitespace run ([`text::has_leading_blank_line`]) rather than its text as a
    /// whole. A total newline count answers a different question and fabricates: trailing text
    /// that merely spans two lines (`…-end -->⏎text1⏎text2`) reaches 2 newlines without the
    /// author writing a blank, and a blank *inside* that text (`…-end -->⏎text1⏎⏎text2`) belongs
    /// to the text — relaying it here would relocate the author's blank onto the seam. The
    /// behavior matches an ordinary preceding comment, which is the parity target.
    fn range_trailing_separator(
        &self,
        nodes: &[FragmentNode<'_>],
        range_end: usize,
    ) -> Option<DocId> {
        if range_end + 1 >= nodes.len() {
            return None;
        }
        let d = self.d();
        let blank = matches!(
            &nodes[range_end + 1],
            FragmentNode::Text(t) if text::has_leading_blank_line(t.raw(self.source))
        );
        Some(if blank {
            d.concat(&[d.literalline(), d.hardline()])
        } else {
            d.hardline()
        })
    }

    /// Detect a "root inline run" starting at `start_idx` in the root fragment.
    ///
    /// A run is a maximal span-adjacent (no whitespace gap in source) sequence that
    /// contains at least one control-flow block **and** at least one content-text or
    /// inline node — e.g. `text{#if}…{/if}text`, `{#each}…{/each}text`, `{x}{#if}…`,
    /// or several chained together. The run renders through the element-content path
    /// (`build_nodes_doc_multiline`) so a directly-adjacent block hugs its text/inline
    /// neighbors (matching prettier and the inside-an-element layout) instead of the
    /// per-node root printer forcing a newline around the block (which would inject
    /// render-significant whitespace — a no-whitespace boundary must never gain a space).
    ///
    /// The walk starts at content text, an inline node, or a control-flow block, and
    /// breaks on whitespace-only text, a non-adjacent node, or any other node (block
    /// element, comment, `{@const}`/`{@debug}`, `{const}`/`{let}`). A content-text node
    /// bridges across the boundary (its content hugs, its own internal/edge newlines
    /// become line breaks in the multiline layout); the run's leading/trailing edge
    /// whitespace is carried as
    /// `pending_ws` by the caller. Lone blocks and pure-inline (no control-flow)
    /// sequences return `None`, keeping the per-node path's behavior for them.
    ///
    /// Returns `Some(end_idx)` (inclusive) for a qualifying run, else `None`.
    fn detect_root_inline_run(
        &self,
        nodes: &[FragmentNode<'_>],
        start_idx: usize,
    ) -> Option<usize> {
        let is_content_text =
            |n: &FragmentNode<'_>| matches!(n, FragmentNode::Text(t) if !t.is_collapsible_ws_only);

        // The start must be a node that can participate in a run.
        let start = &nodes[start_idx];
        if !(is_content_text(start)
            || self.is_inline_run_node(start)
            || nodes::is_control_flow_block(start))
        {
            return None;
        }

        let mut last_idx = start_idx;
        let mut has_control_flow = nodes::is_control_flow_block(start);
        let mut has_content_or_inline = is_content_text(start) || self.is_inline_run_node(start);

        for j in (start_idx + 1)..nodes.len() {
            // Must be directly adjacent (no whitespace gap in source).
            if nodes[j - 1].span().end != nodes[j].span().start {
                break;
            }

            let node = &nodes[j];
            if let FragmentNode::Text(text) = node {
                // Whitespace-only text separates runs (intentional separation).
                if text.is_collapsible_ws_only {
                    break;
                }
                last_idx = j;
                has_content_or_inline = true;
            } else if nodes::is_control_flow_block(node) {
                last_idx = j;
                has_control_flow = true;
            } else if self.is_inline_run_node(node) {
                last_idx = j;
                has_content_or_inline = true;
            } else {
                // Block element, comment, `{@const}`/`{@debug}`, etc. end the run.
                break;
            }
        }

        // Only route runs that genuinely need hugging: a control-flow block plus an
        // adjacent content-text/inline node. A lone block (`last_idx == start_idx`) or a
        // pure-inline / pure-block sequence keeps the per-node path's existing behavior.
        (has_control_flow && has_content_or_inline && last_idx > start_idx).then_some(last_idx)
    }

    /// Check if a fragment node can participate in an inline run (as start or intermediate node).
    fn is_inline_run_node(&self, node: &FragmentNode<'_>) -> bool {
        match node {
            FragmentNode::ExpressionTag(_)
            | FragmentNode::HtmlTag(_)
            | FragmentNode::RenderTag(_) => true,
            FragmentNode::Element(el) => !self.is_block_element(el),
            FragmentNode::SpecialElement(el) => !el.kind.is_block(),
            _ => false,
        }
    }

    /// Format an HTML comment: <!-- content -->
    fn print_comment(&mut self, comment: &internal::HtmlComment) {
        // The hoisted-section (direct-write) emit path for a `<!-- -->` comment, recorded at
        // the write like `tsv_css`'s `print_css_comment`; the template path is the doc-tagged
        // `build_html_comment_doc`. Registered by span in `format_svelte_in`. See
        // `tsv_lang::comment_ledger`.
        #[cfg(feature = "comment_check")]
        tsv_lang::comment_ledger::record_emitted(self.source, comment.span);

        // Verbatim whole span (`<!--…-->`), not re-assembled delimiters — see
        // `build_html_comment_doc`, the doc-path twin.
        self.write(comment.span.extract(self.source));
    }
}
