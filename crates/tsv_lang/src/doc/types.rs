//! Core types for the doc builder

use crate::Span;

/// Group identifier for tracking which groups broke during rendering.
///
/// Enables `indent_if_break` to check if a specific group broke, allowing
/// deferred indentation decisions. Add new variants here as needed.
///
/// Prettier uses Symbol() for unique IDs; we use an enum for type safety.
/// Most formatting needs are handled by `conditional_group` without needing IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupId {
    /// Fluid assignment layout: `a = value`
    /// Used in assignment.rs for conditional right-hand side indentation
    Assignment,
    /// Type parameter `extends` constraint: `<T extends Long>` breaks after
    /// `extends` and indents the constraint when it overflows.
    TypeParameterConstraint,
    /// Type parameter `=` default: `<T = Long>` breaks after `=` and indents
    /// the default when it overflows.
    TypeParameterDefault,
    /// Curried arrow-function chain: the joined signature heads
    /// (`(a) => (b) => …`) break as a unit when they don't fit, and the
    /// terminal body's `indent_if_break` keys on this group so it indents only
    /// when the heads broke.
    ArrowChain,
    /// Svelte block-tag head (`{#if …}`, `{#each …}`, …): the breakable head
    /// expression breaks as a unit when it exceeds print width, and the closing
    /// `}` keys on this group via `if_break` so it dangles on its own line only
    /// when the head broke. The dangle's `if_break` is read immediately after the
    /// head group resolves (before the body), so a shared variant is safe under
    /// block nesting.
    BlockHead,
    /// An `{#each}` key's parens (`… as item (key)}`): the same shape as
    /// [`Self::BlockHead`] one level in, with `)` as the dangling closer. A distinct
    /// variant rather than a shared one because the two are **not** in the read-then-resolve
    /// order that makes `BlockHead` safe under nesting: the key sits inside the head's
    /// clause, which the head group's own `fits()` walks as a rest-command — so a shared id
    /// would have the key's `if_break` consulted while the head's mode is still being
    /// decided.
    BlockKey,
}

impl GroupId {
    /// Number of variants. Sizes the renderer's inline `[Option<Mode>; COUNT]`
    /// group-mode map (indexed by `id as usize`), which replaces a per-render
    /// `HashMap`. Keep in sync when adding a variant — a stale (too-small) value
    /// would index out of bounds, caught immediately by the fixture suite.
    pub(crate) const COUNT: usize = 7;
}

/// Context for doc rendering - provides hints about trailing punctuation
/// that affect how content is rendered.
///
/// This allows fills to make better packing decisions by knowing about
/// punctuation that will be added by the parent (e.g., semicolons in CSS,
/// commas in object properties).
///
/// The layout flags are deliberate per-fill render policies set by the language
/// printers (Svelte boundary rules, CSS trailing punctuation), packed into one
/// private flag word — read through the named getters ([`Self::glued_lead`], …),
/// set through the matching `with_*` builders.
#[derive(Debug, Clone, Default)]
pub struct DocContext {
    /// Reserved trailing columns — [`Self::trailing_reserve`] carries the contract,
    /// [`Self::reserving`] is the sole setter.
    trailing_reserve: u16,

    /// The per-fill layout flag bits — one packed word rather than `bool` fields, so a new flag
    /// never moves `DocNode`'s size (the note below the struct carries the budget). Read through
    /// the named getters ([`Self::break_before_wide_flow`], [`Self::after_element_fold`],
    /// [`Self::glued_lead`], [`Self::glued_atom`] — each carries its flag's full contract), set
    /// through the matching `with_*` builders.
    flags: u16,
}

// `DocContext` is stored **inline** in `DocNode::WithContext`, whose size is `const`-asserted per
// target — 16 B on wasm32, the tightest budget and the one the shipped npm packages build under.
// Nothing in `deno task check` builds wasm32, so without this assert a context that outgrows the
// budget passes every gate and fails only at `deno task build:packages` (it already did once:
// growing the context blew the `DocNode` assert, invisibly to the whole gate chain).
//
// This pins the cost **here**, natively, where a field would be added. `WithContext`'s payload is
// `DocId` + this struct = 8 B, comfortably under wasm32's 16 B budget with room for the enum tag.
// The layout flags are one packed `u16` (11 bits free), so a new flag is size-free until the
// seventeenth; only a new *field* (or widening `trailing_reserve`) can move this number, and it
// has ~8 B of slack before the wasm32 `DocNode` assert blows.
//
// Note this is the ONLY budget in play — `WithContext` is not what sizes `DocNode` on either
// target. See the `DocNode` size assert in `arena.rs` for the variants that actually are.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<DocContext>() == 4);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(size_of::<DocContext>() == 4);

impl DocContext {
    const BREAK_BEFORE_WIDE_FLOW: u16 = 1 << 0;
    const AFTER_ELEMENT_FOLD: u16 = 1 << 1;
    const GLUED_LEAD: u16 = 1 << 2;
    const GLUED_ATOM: u16 = 1 << 3;
    const FLOW_BREAK_PROBE: u16 = 1 << 4;
    const HOLD_LINE_AFTER_BROKEN_FLOW: u16 = 1 << 5;

    /// A context that only reserves `columns` trailing columns — the CSS trailing-punctuation
    /// case, and the sole reason [`Self::trailing_reserve`] exists.
    ///
    /// Callers hold their widths as `usize`, so the clamp lives here rather than at each of them.
    /// It is unobservable: a reserve at all near `u16::MAX` already means "nothing fits", which is
    /// what saturating produces.
    #[must_use]
    pub fn reserving(columns: usize) -> Self {
        Self {
            trailing_reserve: u16::try_from(columns).unwrap_or(u16::MAX),
            ..Self::default()
        }
    }

    /// Reserve N chars when checking if content fits.
    ///
    /// This prevents greedy fills from packing to exactly printWidth,
    /// which would be exceeded when the parent adds trailing punctuation.
    ///
    /// Example: CSS declarations add ";" after the value, so reserve 1 char.
    ///
    /// `u16`, not `usize`: this is a **column count**, bounded in practice by print width, and
    /// `DocContext` is stored *inline* in `DocNode::WithContext`, whose size is `const`-asserted
    /// per target (see the note beneath the struct). Widening it spends the budget that the
    /// layout flags need.
    #[inline]
    #[must_use]
    pub const fn trailing_reserve(&self) -> u16 {
        self.trailing_reserve
    }

    #[inline]
    const fn flag(&self, bit: u16) -> bool {
        self.flags & bit != 0
    }

    #[inline]
    #[must_use]
    const fn with_flag(mut self, bit: u16, on: bool) -> Self {
        if on {
            self.flags |= bit;
        } else {
            self.flags &= !bit;
        }
        self
    }

    /// When set, the fill's trailing separator (its terminal `line`, the only one reaching the
    /// "content + separator" render case) measures the *immediately following* node — the next
    /// item on the render stack — as a WHOLE flat unit, instead of letting that node's own internal
    /// break point short-circuit the fit check. A wide inline element that would not fit flat after
    /// the separator then forces the separator to break, dropping the element to its own line whole
    /// — rather than packing it onto the text line, where it would break its own tag in place.
    ///
    /// When that following node is an *after-element fold* (an inline element + its trailing text,
    /// carrying [`Self::after_element_fold`]), only the fold's **lead element** is measured, not the
    /// whole element+tail unit — the tail can wrap, so a short element packs after the last word
    /// instead of dropping (prettier's fill is pairwise: last word, separator, element — never the
    /// tail). See
    /// [`crate::doc::arena::DocArena::welded_atom`].
    ///
    /// Pairwise cuts on the other side too: the measurement STOPS at that following node (plus any
    /// run welded to it, which owns no break point of its own and so rides its line by
    /// construction), rather than running the whole render stack. A later sibling the fill reaches
    /// only across a break point of its own does not belong in the element's fit check. The stack
    /// is built by `flow_lookahead` in `arena_render_fill`, which both halves of the boundary rule
    /// share.
    ///
    /// Scoped to the Svelte text→flow boundary fill — a text run whose next sibling is a flowing
    /// inline element/component, or a `{expr}` / `{@html}` / `{@render}` tag. A TAG follower's
    /// doc is measured exactly like an element's (forced flat, its width the formatted
    /// expression's), on **both** boundary shapes, unconditionally: glued, the welded word+tag
    /// pair is the smallest welded unit, travelling rather than riding past print width; spaced,
    /// a tag whose expression cannot fit flat travels past the separator instead of opening
    /// mid-line (conformance_prettier.md §Print Width Philosophy). There is deliberately no
    /// build-side follower condition — how far the unit extends past the follower is decided at
    /// render by `flow_lookahead`'s welded walk over the built docs, the single authority on
    /// unit extent. Off for every other fill, so a small element after text still packs
    /// and CSS/value-list fills are unaffected. It re-couples the width-driven drop decision to
    /// the boundary rule at render position so the space- and newline-authored forms converge to
    /// one fixed point.
    ///
    /// **One flag, both boundary shapes**, distinguished only by whether a separator sits between
    /// the last word and the following element — the fill's own parity routes each to the right
    /// render case, so the same flag drives both without cross-talk:
    /// - **space-separated** (`… word <a…>`, a trailing `line`): the element is the fill's Case-2
    ///   separator target; the whole-flat measurement lands in Case 2's `sep_fits`.
    /// - **glued** (`… glued<a…>`, no whitespace, no trailing separator): the glued word is the
    ///   fill's Case-1 last item and the element follows on the render stack; the same whole-flat
    ///   measurement lands in Case 1's `content_fits`. Without it Case 1 inherits Break mode into
    ///   the element and short-circuits at its first internal line (wrongly "fits"), welding the
    ///   word and breaking the element's own content in place. Measuring the following run as a
    ///   whole flat unit breaks at the whitespace boundary BEFORE the glued word so the whole run
    ///   (`glued<a>…</a>`) moves to a fresh line together — never splitting the glued boundary,
    ///   which would inject a rendered space.
    ///
    /// A ws-fill also reaches the Case-1 measurement at `is_final_segment` (its last word), but
    /// there the measured content is a bare word whose only consumer is `should_remeasure`, inert
    /// for a groupless leaf — so the shared flag stays free of cross-case contamination.
    #[inline]
    #[must_use]
    pub const fn break_before_wide_flow(&self) -> bool {
        self.flag(Self::BREAK_BEFORE_WIDE_FLOW)
    }

    /// Returns `self` with [`Self::break_before_wide_flow`] set to `on`.
    #[inline]
    #[must_use]
    pub const fn with_break_before_wide_flow(self, on: bool) -> Self {
        self.with_flag(Self::BREAK_BEFORE_WIDE_FLOW, on)
    }

    /// When set, this fill **is** the Svelte after-element fold (`fill([element, line, words…])`,
    /// built only by `build_after_element_fold`) — an inline element followed by its terminal
    /// trailing text. Unlike the other flags this one states the fill's *identity*, not a policy;
    /// two render behaviors and one shape query follow from it, and they are one flag rather than
    /// three because there is exactly one construction site and no fill wants a subset:
    ///
    /// - **Head hug.** The fold's FIRST item — always a breakable inline element/component — is a
    ///   breakable atom the fill must not drop. Sitting mid-line right after a small prefix (a
    ///   parent inline element's `>`) and not fitting on its own line *either* (wider than
    ///   printWidth even at line start), it renders **in place** and breaks internally, rather than
    ///   dropping to the next line. Dropping a too-wide-anyway head only strands a spurious break
    ///   before it (a `>⏎<child` dangle the next pass collapses → non-idempotent); rendering in
    ///   place keeps the child hugging the prefix, matching the newline-authored form.
    /// - **Terminal tail hug.** Once the head has wrapped at line start, the terminal trailing text
    ///   hugs the dangled closing `>` (`</tag⏎> tail`) if it fits there, instead of taking its own
    ///   line. The separator after the wrapped head is chosen by the actual resulting column — flat
    ///   (hug) when the next item still fits, else break. This is how tsv respects an author's
    ///   *space* boundary after a wide inline element, mirroring how short inline elements already
    ///   keep `<el>x</el> tail` inline; a *newline*-authored boundary still takes its own line (the
    ///   text node carries the newline, so it never reaches this fold).
    /// - **Lead extraction.** [`crate::doc::arena::DocArena::welded_atom`] recognizes
    ///   the fold by this flag and returns its head, so a *preceding* text run's
    ///   [`Self::break_before_wide_flow`] measurement grades the element alone rather than the whole
    ///   element+tail unit.
    ///
    /// Off for every other fill: text word-wrap and CSS value lists still drop a too-wide item onto
    /// its own line, and a wrapped item never lets the next hug its last line.
    #[inline]
    #[must_use]
    pub const fn after_element_fold(&self) -> bool {
        self.flag(Self::AFTER_ELEMENT_FOLD)
    }

    /// Returns `self` with [`Self::after_element_fold`] set to `on`.
    #[inline]
    #[must_use]
    pub const fn with_after_element_fold(self, on: bool) -> Self {
        self.with_flag(Self::AFTER_ELEMENT_FOLD, on)
    }

    /// When set, the fill's FIRST item is **byte-glued** to whatever precedes it on the render
    /// stack, so the boundary before it carries no whitespace. The fill therefore never moves that
    /// item to a fresh line when it doesn't fit mid-line: it renders in place (prettier's shape) and
    /// breaks at the first whitespace boundary *inside* the run instead, even when the glued head
    /// overruns printWidth. Only the fill's head is affected — every later item is separated by real
    /// whitespace and breaks normally.
    ///
    /// This is the mirror of [`Self::break_before_wide_flow`]'s glued half, on the other side of the
    /// run: there a text run is glued to a *following* element and the break travels to the
    /// whitespace before the run; here the run is glued to a *preceding* node (a Svelte
    /// `<!--c-->text` boundary) and there is no whitespace before it to travel to. Breaking anyway
    /// would inject a rendered space — and the mangled form is a fixed point, so F1 cannot see it.
    ///
    /// Two Svelte fills set it, and they are the two ways a run can acquire a glued head: a
    /// **text-run** fill whose leading boundary is glued (a `<!--c-->text` boundary), and the
    /// **after-element fold**, whose head is the inline element the terminal tail packs after
    /// (`.w<b>y</b> tail` — the `<b>` welded to the text before it). The second is why this flag
    /// composes with [`Self::after_element_fold`] rather than excluding it: the fold states what
    /// the fill *is*, this states what its head may not do. Off for every other fill, so the
    /// ordinary fresh-line drop (text word-wrap, CSS value lists) is unaffected.
    ///
    /// It composes with [`Self::break_before_wide_flow`] too — a MID-RUN welded fill (glued at
    /// its head, ending before a flow follower: `… <b>a</b>glued {x}<i>…`) carries both, and a
    /// third reader keys on this flag from *outside*: a preceding boundary's welded walk
    /// ([`crate::doc::arena::DocArena::welded_entry`]) reads it to extend that boundary's
    /// measured unit THROUGH this fill. The three consumers are disjoint — the head's drop
    /// suppression (this flag, Case 3 at `offset == 0`), the trailing measurement
    /// ([`Self::break_before_wide_flow`], Cases 1/2 at `is_final_segment`), and the upstream
    /// walk (entry classification) — so the flags never contend, but a builder that WRAPS a
    /// fill carrying them hides all three at once (the marker-burial hazard — a wrapping
    /// builder must re-hoist the marker's flags onto the wrapper).
    ///
    /// ⚠️ The head's drop suppression is **two** render sites, not one: Case 3's arm and Case 1's,
    /// the latter reached whenever the run is a fill of a single item (`is_glued_head` in
    /// `arena_render_fill` is the shared predicate). Note the upstream walk reads this flag off a
    /// non-`Fill` too — an element doc marked `glued_lead` + [`Self::glued_atom`] — which is why it
    /// is exempt from the render-channel tripwire in
    /// [`crate::doc::arena::DocArena::with_context`].
    #[inline]
    #[must_use]
    pub const fn glued_lead(&self) -> bool {
        self.flag(Self::GLUED_LEAD)
    }

    /// Returns `self` with [`Self::glued_lead`] set to `on`.
    #[inline]
    #[must_use]
    pub const fn with_glued_lead(self, on: bool) -> Self {
        self.with_flag(Self::GLUED_LEAD, on)
    }

    /// Set beside [`Self::glued_lead`] when the glued doc is a breakable **atom** — an inline
    /// element, or a glued element run — rather than a text run. It exists only so a flow-boundary
    /// look-ahead can tell the two apart ([`crate::doc::arena::DocArena::welded_atom`]): an atom is
    /// measured **flat**, a text run rides in its own mode; the walk continues past either while
    /// the next entry is still glued, and the unit ends at the first entry that is not.
    ///
    /// ⚠️ It is a separate flag because the distinction is **not** recoverable from the doc's shape.
    /// A single-word text run is a bare `Text` (`build_text_fill_doc_trimmed` returns the word
    /// itself, not a `Fill`), and one carrying a glued prefix is a `Concat` — so "the context wraps
    /// a non-`Fill`" reads a single-word `.w` as an atom, ends the walk early, and the run it was
    /// supposed to measure never reaches the boundary's fit check. That was a real regression; do
    /// not replace this flag with a node-kind sniff.
    ///
    /// On such a marker the context is otherwise **inert**: the renderer applies a `DocContext` only
    /// when it wraps a `Fill`, and this wraps an element, so the mark costs one node and changes no
    /// layout by itself.
    #[inline]
    #[must_use]
    pub const fn glued_atom(&self) -> bool {
        self.flag(Self::GLUED_ATOM)
    }

    /// Returns `self` with [`Self::glued_atom`] set to `on`.
    #[inline]
    #[must_use]
    pub const fn with_glued_atom(self, on: bool) -> Self {
        self.with_flag(Self::GLUED_ATOM, on)
    }

    /// When set, rendering this node records whether its subtree actually emitted a line
    /// break: the renderer snapshots the output length on entry, pushes a
    /// [`super::arena::DocNode::FlowProbeEnd`] sentinel behind the subtree, and the sentinel
    /// stores "the subtree's output contained a newline" as the arena's most-recent flow-probe
    /// answer. Paired with [`Self::hold_line_after_broken_flow`] on the *immediately following*
    /// doc — a text tail's fill, or the held inline-sibling wrap's leading line — the two are
    /// built together by the Svelte authored-newline boundary rule, and the pairing is
    /// positional: the sentinel completes right before the paired doc renders, so the answer
    /// cannot be stale. Invisible to measurement (`arena_fits` skips the sentinel), so
    /// flagging a doc never changes any fit decision — the whole point, after a
    /// `group([element, line])` join was measured through and re-broke the *preceding*
    /// boundary (the razor-caught 2-cycle this replaced).
    #[inline]
    #[must_use]
    pub const fn flow_break_probe(&self) -> bool {
        self.flag(Self::FLOW_BREAK_PROBE)
    }

    /// Returns `self` with [`Self::flow_break_probe`] set to `on`.
    #[inline]
    #[must_use]
    pub const fn with_flow_break_probe(self, on: bool) -> Self {
        self.with_flag(Self::FLOW_BREAK_PROBE, on)
    }

    /// When set on a fill, its LEADING separator (a collapsible line in the first content
    /// slot — the `leading_line` parity) renders as a forced break when the flow probe's
    /// most-recent answer ([`Self::flow_break_probe`]) says the probed predecessor rendered
    /// multiline; otherwise the fill renders exactly as an unflagged one. Set on a bare `Line`
    /// — the leading line of the held inline-sibling wrap,
    /// [`crate::doc::arena::DocArena::inline_sibling_line_group_held`] — it is the same hook
    /// read by the renderer's `WithContext` arm instead of the fill loop. This is the Svelte
    /// authored-newline boundary rule's render half: `</a>⏎text` and `</a>⏎<b>x</b>` keep the
    /// tail's own line beside an element that actually rendered multiline, and reflow beside
    /// one that rendered inline — layout-keyed at render, with no build-side prediction and no
    /// measurement change (an outer fits walk sees an ordinary fill or wrap whose leading line
    /// is an ordinary break opportunity).
    #[inline]
    #[must_use]
    pub const fn hold_line_after_broken_flow(&self) -> bool {
        self.flag(Self::HOLD_LINE_AFTER_BROKEN_FLOW)
    }

    /// Returns `self` with [`Self::hold_line_after_broken_flow`] set to `on`.
    #[inline]
    #[must_use]
    pub const fn with_hold_line_after_broken_flow(self, on: bool) -> Self {
        self.with_flag(Self::HOLD_LINE_AFTER_BROKEN_FLOW, on)
    }
}

/// Sentinel value for cached_width: text contains a newline.
/// Used by fits to early-return without resolving the string.
pub const TEXT_WIDTH_HAS_NEWLINE: u16 = u16::MAX;

/// Sentinel value for cached_width: width not yet computed.
/// Used for owned strings that may be expensive to measure upfront.
pub const TEXT_WIDTH_NOT_COMPUTED: u16 = u16::MAX - 1;

/// A slice into the [`super::arena::DocArena`]'s text pool — the arena-owned `String`
/// holding every dynamically-built text body ([`DocText::Pooled`],
/// [`super::arena::DocNode::MultilineText`]). Offsets are byte indices into that pool,
/// resolved at render time against the pool borrowed from the same arena the
/// node lives in (the pool-keyed sibling of the source-keyed
/// [`DocText::SourceSpan`]). Storing a span instead of an owned `String` keeps
/// `DocNode` free of drop glue, so the arena's `reset()`/drop never walk the
/// node store running destructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolSpan {
    /// Byte offset of the text's start in the arena text pool.
    pub start: u32,
    /// Byte length of the text.
    pub len: u32,
}

impl PoolSpan {
    /// Resolve to the text slice within `pool` (the owning arena's text pool).
    #[inline]
    pub fn slice(self, pool: &str) -> &str {
        &pool[self.start as usize..(self.start + self.len) as usize]
    }
}

/// Text content in a Doc - a static string, a pooled owned string, or a source span resolved at print time
#[derive(Debug, Clone)]
pub enum DocText {
    /// Static string literal - no allocation, just stores pointer.
    /// Second field is the precomputed visual width (a real width or
    /// [`TEXT_WIDTH_HAS_NEWLINE`], never [`TEXT_WIDTH_NOT_COMPUTED`]) —
    /// amortized through the arena's static width cache, measured once per
    /// unique string per arena rather than per node.
    Static(&'static str, u16),
    /// Dynamically generated text, stored in the arena's text pool — the
    /// drop-glue-free replacement for a per-node owned `String`. Resolved
    /// against the pool at render time (like `SourceSpan` against `source`).
    /// Second field is the precomputed visual width — **always** computed at
    /// build (a real width or [`TEXT_WIDTH_HAS_NEWLINE`], never
    /// [`TEXT_WIDTH_NOT_COMPUTED`]), so the fits walk never needs the pool:
    /// width queries answer from the node alone, and only the render loop
    /// (which borrows the pool once per render) reads the bytes. Pooled text
    /// is rare (~1.4% of Text nodes), so the eager measure is off the hot
    /// path by construction.
    Pooled(PoolSpan, u16),
    /// Verbatim source slice, resolved against `source` at print time. Second
    /// field is the precomputed visual width — always computed at build like
    /// `Pooled` (a real width or [`TEXT_WIDTH_HAS_NEWLINE`]), identifier and
    /// element/attribute names included: the eager policy has no exceptions
    /// (rationale, and the measured cost of the deferral names used to take, on
    /// the arena's `pooled_text_width`). Lets a printer emit
    /// verbatim source text (identifier/tag/attribute names, comments, template
    /// chunks, already-canonical literals) with **no allocation and no copy** —
    /// the lifetime-free alternative to a borrowed `&'src str` (which would force
    /// `DocArena<'src>` and forfeit the cross-file arena `reset()` reuse). The
    /// span is resolved at render against the document source threaded through
    /// [`resolve_text`]; behaves identically to the pooled text it replaces in
    /// every doc transform (a `DocNode::Text` is matched generically).
    SourceSpan(Span, u16),
    /// A format-ignored **verbatim slice** (the `prettier-ignore` freeze) —
    /// [`SourceSpan`](DocText::SourceSpan) in every mechanical respect (same
    /// eager width policy, same render resolution against `source`), but
    /// **layout-opaque**: `will_break` does not report its embedded newlines as
    /// a forced break. A frozen slice's newlines are *source* layout, not a
    /// break the enclosing group must honor — prettier's `printIgnored` output
    /// is a plain string doc its willBreak/propagateBreaks never see, and the
    /// enclosing containers lay out as if the slice were flat. `fits()` is
    /// unaffected (it keys on the width slot, where the newline sentinel still
    /// ends the measured line). Built only via `verbatim_source_span`; genuine
    /// multi-line content (line-continuation strings, `<pre>` text) must stay
    /// [`SourceSpan`](DocText::SourceSpan) so it force-breaks.
    VerbatimSpan(Span, u16),
}

impl DocText {
    /// Get the cached visual width.
    ///
    /// Decodes the stored `u16` (a real width or one of the two sentinel
    /// values) into [`CachedWidth`], so callers can't mistake
    /// [`TEXT_WIDTH_HAS_NEWLINE`] for an actual width — every consumer must
    /// handle the newline case explicitly.
    #[inline]
    pub const fn cached_width(&self) -> CachedWidth {
        match self {
            DocText::Static(_, w)
            | DocText::Pooled(_, w)
            | DocText::SourceSpan(_, w)
            | DocText::VerbatimSpan(_, w) => match *w {
                TEXT_WIDTH_NOT_COMPUTED => CachedWidth::NotComputed,
                TEXT_WIDTH_HAS_NEWLINE => CachedWidth::HasNewline,
                w => CachedWidth::Width(w),
            },
        }
    }
}

/// Decoded form of a [`DocText`] width slot — see [`DocText::cached_width`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedWidth {
    /// Precomputed single-line visual width.
    Width(u16),
    /// The text contains a newline — there is no single-line width; fits
    /// treats the line as ending inside this text.
    HasNewline,
    /// Not precomputed — measure on demand. No builder emits this today (the
    /// eager width policy on the arena's `pooled_text_width` has no
    /// exceptions); it is the mechanism a deferral would use, and what the
    /// `arena_fits` on-demand oracle grades.
    NotComputed,
}

/// Resolve DocText to a string, against the document source if provided.
///
/// For Static text, returns directly. For Pooled text, slices the arena text
/// pool the caller borrowed (render hoists it once per render). For a
/// SourceSpan, slices the document `source` (panics if `source` is None).
///
/// # Panics
///
/// Panics if a SourceSpan is encountered but no `source` was provided. This
/// indicates a bug — docs containing source spans must use resolved print
/// functions (the ones threading the document source).
///
/// ⚠️ **`inline(always)`, not `inline`, and the difference is measured.** The
/// body reads as a four-arm match handing back a slice, but three of those arms
/// index a `str` by range, and each range index carries two `is_char_boundary`
/// probes plus an edge to `slice_error_fail` — so the code is an order of
/// magnitude larger than the match, LLVM declined the plain `#[inline]` hint,
/// and this was left out of line and **called**: once per rendered `Text` node
/// from `render_text`, whose very next statement re-matches the same
/// `DocText` for its cached width, and once per unmeasured identifier name from
/// `text_flat_width`. Forcing it in measures `instructions:u` **−1.39…−1.58%**
/// across five real corpora and **−0.96%** on pure CSS, against two
/// provably-unreachable null controls at **±0.000%**, for **+640 B** of
/// `.text`. The render write is the whole of it — forcing that site alone reads
/// −1.514% where both together read −1.516% — so the fits walk keeps its inline
/// resolve for uniformity, not for a share.
#[expect(clippy::inline_always)]
#[inline(always)]
#[expect(clippy::expect_used)] // Intentional: SourceSpan without source is a programming error
pub(super) fn resolve_text<'a>(
    text: &'a DocText,
    source: Option<&'a str>,
    pool: &'a str,
) -> &'a str {
    match text {
        DocText::Static(s, _) => s,
        DocText::Pooled(span, _) => span.slice(pool),
        DocText::SourceSpan(span, _) | DocText::VerbatimSpan(span, _) => span.extract(
            source.expect("SourceSpan encountered in Doc but no source provided for resolution"),
        ),
    }
}

/// Line break behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Normal line: space in flat mode, newline + indent in break mode
    Normal,
    /// Soft line: disappears in flat mode, newline + indent in break mode
    Soft,
    /// Hard line: always breaks with newline + indent (ignores flat mode)
    Hard,
    /// Literal line: always breaks with newline only, NO indentation
    /// Used for blank line preservation
    Literal,
}

/// Rendering mode for a doc
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Try to fit on one line (soft lines become spaces)
    Flat,
    /// Use line breaks (soft lines become newlines)
    Break,
}
