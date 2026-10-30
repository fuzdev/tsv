//! Arena-based document allocation for efficient Doc tree construction and rendering.
//!
//! Instead of heap-allocating each Doc node individually (`Box<Doc>`, `Vec<Doc>`),
//! all nodes are stored in a contiguous `Vec<DocNode>` and referenced by `DocId`
//! (a u32 index). Child lists are stored in a separate flat `Vec<DocId>` and
//! referenced by `ChildRange { start, len }`.
//!
//! Benefits:
//! - No recursive drop, no per-node destructors — `DocNode` carries no drop
//!   glue (dynamic text lives in the arena text pool), so clearing or dropping
//!   the arena never walks the node store
//! - No deep cloning (DocId is Copy)
//! - Cache-friendly contiguous storage
//! - Bulk deallocation

use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;

use smallvec::SmallVec;

use crate::Span;
use crate::config::TAB_WIDTH;
use crate::hash::FxHashMap;
use crate::printing::{
    next_lf, next_width_relevant, next_width_relevant_in, visual_width, visual_width_mixed,
};

#[cfg(feature = "comment_check")]
use crate::comment_ledger::{DocumentKey, comment_check_enabled, document_key};

use super::DocBuf;
use super::chain_share::ChainShareStore;
#[cfg(feature = "swallow_check")]
use super::swallow::swallow_check_enabled;
use super::types::{
    CachedWidth, DocContext, DocText, LineKind, Mode, PoolSpan, TEXT_WIDTH_FIRST_LINE_MASK,
    TEXT_WIDTH_NEWLINE_FLAG,
};

/// Which **prettier operation** a line-flattening walk is emulating.
///
/// Two different operations wear one walk, and every behavioral difference between them
/// follows from this choice — so name the operation, not any one of its symptoms:
///
/// | | [`Self::RemoveLines`] | [`Self::Atomize`] |
/// | --- | --- | --- |
/// | emulates | `removeLines` (`document/utilities`) | `printDocToString` at `printWidth: Infinity` |
/// | entry point | [`DocArena::remove_lines`] | [`DocArena::atomize`] |
/// | hard / literal lines, `MultilineText` | kept (prettier's `!doc.hard` gate) | deleted |
/// | `conditional_group` | states kept | collapsed to the least-expanded state |
///
/// The hard-line axis is the dangerous one: deleting a hard line does not relayout
/// anything, it deletes a newline the content **required**, so [`Self::Atomize`] is only
/// sound where the caller has proved none is required.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FlattenMode {
    /// Prettier's `removeLines`: statically flatten breakable lines only.
    RemoveLines,
    /// Force onto one line at any width — what a re-render at infinite width would print.
    Atomize,
}

/// The doc levels [`DocArena::flatten_lines_impl`] walks by recursion, a frame each, before
/// the subtree below is walked by [`DocArena::flatten_lines_loop`]: the common flattened doc
/// is a few levels deep and its recursion is the cheaper walk, while a deeper one costs the
/// stack of this many frames whatever its depth — a doc nests a level per link of some
/// chains (a run of commented `!`), so its depth is bounded only by `SPINE_LIMIT`.
const FLATTEN_FRAMES: u32 = 32;

/// What [`DocArena::flatten_build`] reads of one node ([`DocArena::flatten_info`]): its
/// kind and the ids of the children the copy is built from.
enum FlattenInfo {
    /// Copied as itself.
    Keep,
    FlattenedMultilineText(String),
    Line(LineKind),
    Indent(DocId),
    Dedent(DocId),
    AlignRoot(usize, DocId),
    Align(u32, DocId),
    Group {
        contents: DocId,
        expanded_states: ChildRange,
        should_break: bool,
    },
    IfBreakFlat(DocId),
    IndentIfBreakContents(DocId),
    Concat(DocBuf),
    Fill(DocBuf),
    WithContext(DocId, DocContext),
    LineSuffix(DocId),
    BreakParent,
    GatedState(DocId, DocId, GateAdmit),
}

impl FlattenInfo {
    /// Whether the copy takes the original's comment-ledger tag
    /// ([`DocArena::flatten_retag`]): every rebuilt node's does but an atomized group's,
    /// whose copy the atomizing arm hands back untagged.
    #[cfg(feature = "comment_check")]
    fn retags(&self, mode: FlattenMode) -> bool {
        !(mode == FlattenMode::Atomize && matches!(self, Self::Group { .. }))
    }
}

/// Index into `DocArena.nodes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DocId(u32);

impl DocId {
    /// Get the raw index value.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A keyed group's identity — prettier's per-instance group id (`Symbol("…")`): the group
/// node's own [`DocId`], so two groups built by the same printer arm are two ids, and a
/// reader keyed on one can never read the other's mode.
///
/// Minted only by [`DocArena::group_with_id`] / [`DocArena::group_with_id_break`], which
/// allocate the group it names; [`Self::doc`] is the node to place in the tree, and the
/// readers ([`DocArena::if_break_with_id`], [`DocArena::indent_if_break`]) take the id.
/// Held as the bare index — node 0 is the prelude's empty text (`seed_prelude`), never a
/// group — so `Option<GroupId>` is four bytes (a guarantee, through `#[repr(transparent)]`
/// over a `NonZeroU32`) and a keyed `IfBreak` stays within [`DocNode`]'s size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct GroupId(NonZeroU32);

const _: () = assert!(size_of::<Option<GroupId>>() == 4);

impl GroupId {
    /// The id of the group node at `doc`. Called once per keyed group, at build time — the
    /// render keys its map on the bare [`DocId`] and never mints an id.
    #[inline]
    fn of(doc: DocId) -> Self {
        // Every arena seeds its prelude before its first allocation, so a group is never
        // node 0; the check is one predictable branch per keyed group.
        #[expect(clippy::expect_used)]
        let raw =
            NonZeroU32::new(doc.0).expect("node 0 is the prelude's empty text, never a group");
        Self(raw)
    }

    /// The group node this id names — the doc to place in the tree.
    #[inline]
    pub const fn doc(self) -> DocId {
        DocId(self.0.get())
    }
}

/// Range into `DocArena.children` for multi-child nodes (Concat, Fill, expanded_states).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildRange {
    pub start: u32,
    pub len: u32,
}

impl ChildRange {
    /// An empty range (no children).
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    /// Check if the range is empty.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Resolve to a slice of DocIds.
    #[inline]
    pub fn resolve(self, children: &[DocId]) -> &[DocId] {
        &children[self.start as usize..(self.start + self.len) as usize]
    }
}

/// Arena-allocated document node.
///
/// Stores children as `DocId` indices and child lists as `ChildRange` ranges.
///
/// ⚠️ **VARIANT ORDER IS LOAD-BEARING, and no test can pin it.** Two separate
/// mechanisms read the discriminants as a range:
///
/// - [`DocText`]'s four sub-tags own tags 0..=3 (the niche that holds a
///   `DocNode` at 24 B), so `Text` is the fold every `match` here computes —
///   which is why probing `Text` ahead of a dispatch is free and probing a kind
///   above the fold is not (see `docs/architecture.md`).
/// - [`DocArena::subtree_layout_memo`] peels the run **immediately above** that
///   niche — `MultilineText` through `Group` — so its second test is one
///   unsigned compare and a `Concat` falls through on a direct branch. A set
///   with a hole in it lowers to a jump table and charges that fall-through an
///   indirect jump: measured, the same lever is −0.173% instead of −0.377%.
///
/// So moving a variant is a performance change with no other signal. Re-measure
/// all three doc walks (render, fits, layout) before reordering.
#[derive(Debug, Clone)]
pub enum DocNode {
    /// Text content to output (static, pooled, or source-span)
    Text(DocText),

    /// Multi-line text rendered with per-line context indent.
    ///
    /// Holds a body whose lines are `\n`-separated in the arena text pool. The
    /// first line renders at the current column; every subsequent line is
    /// preceded by a context-indented hardline (trim trailing whitespace,
    /// newline, write indentation). Output- and position-identical to
    /// `concat([text(line0), hardline, text(line1), hardline, …])`, but stores
    /// the whole body contiguously instead of one node (and one text
    /// allocation) per line.
    ///
    /// `first_width` is the precomputed visual width of the first line
    /// (clamped like every cached text width — see [`pooled_text_width`]),
    /// so the fits walk measures the node without touching the pool.
    ///
    /// Used for multi-line block comments. An indentable one (JSDoc / `*`-aligned)
    /// takes the uniform hardline (context-indent) layout on every continuation
    /// line; a non-indentable one sets `literal`, and each `\n` then renders as a
    /// `literalline` instead — a bare newline, no trim and no indent — so the
    /// comment's interior columns are kept exactly as authored. Always contains a
    /// newline, so it forces enclosing groups to break (`will_break` is true)
    /// exactly like the hardlines it replaces.
    ///
    /// One node for the whole comment is what lets the renderer treat it as ONE
    /// token: a pending `line_suffix` flushes AHEAD of it rather than at an
    /// interior break, where it would land inside the comment
    /// (`arena_render::flush_suffix_ahead_of_multiline_text`). The per-line concat
    /// this replaced flushed at those breaks and welded the two comments.
    MultilineText {
        span: PoolSpan,
        first_width: u16,
        literal: bool,
    },

    /// Line break - behavior depends on kind and mode
    Line(LineKind),

    /// Increase indentation level for nested content
    Indent(DocId),

    /// Decrease indentation level
    Dedent(DocId),

    /// Reset to an absolute whole-tab indentation level (the template-literal
    /// root reset — Prettier's `dedentToRoot`; the only production use is level
    /// `0`). Distinct from [`Align`](DocNode::Align), the sub-tab space offset.
    AlignRoot { n: usize, contents: DocId },

    /// Sub-tab alignment offset of `n` literal spaces — Prettier's numeric
    /// `align(n, …)` (`document/builders/align.js`). Under `useTabs` this is
    /// rendered as *spaces*, not a whole tab — so alignment stays tab-width
    /// independent — and it rounds up to a whole tab only when a further
    /// [`Indent`](DocNode::Indent) is stacked on top of it (Prettier's
    /// `generateIndent` flush). Distinct from [`AlignRoot`](DocNode::AlignRoot),
    /// which sets an absolute *tab* level.
    Align { n: u32, contents: DocId },

    /// Try to fit content on one line; if doesn't fit, break ALL lines in group.
    ///
    /// When `expanded_states` is non-empty, this is a "conditional group" that tries
    /// multiple alternative layouts. `contents` is `state[0]`, `expanded_states` contains
    /// state[1..].
    ///
    /// `keyed` marks a group some reader keys on ([`GroupId`] — the group's own node): the
    /// render records its resolved mode under that id for the readers to find.
    Group {
        contents: DocId,
        expanded_states: ChildRange,
        keyed: bool,
        should_break: bool,
    },

    /// Conditional rendering based on whether a group breaks.
    ///
    /// `group_id == None` keys on the immediately enclosing group (the common
    /// case). `group_id == Some(id)` keys on one specific group's resolved mode
    /// (like `IndentIfBreak`), so the conditional can react to a group it is not
    /// nested inside — e.g. a block-tag head's `}` dangling after its head group.
    IfBreak {
        break_doc: DocId,
        flat_doc: DocId,
        group_id: Option<GroupId>,
    },

    /// Conditionally indent based on whether one specific group broke.
    IndentIfBreak { contents: DocId, group_id: GroupId },

    /// Sequence of docs - rendered one after another
    Concat(ChildRange),

    /// Greedy line packing - fills each line with as much as fits
    Fill(ChildRange),

    /// Wrap a doc with rendering context (hints for width/punctuation)
    WithContext { doc: DocId, context: DocContext },

    /// Content to print at the end of the current line
    LineSuffix(DocId),

    /// Force any pending LineSuffix content to be flushed
    LineSuffixBoundary,

    /// The END of an embedded document: past this node the line belongs to the HOST
    /// (a Svelte template island's closer, a block head's `as`), so a
    /// [`LineSuffix`](DocNode::LineSuffix) still pending here has no line end of this
    /// document left to flush at.
    ///
    /// Render: with a suffix pending it flushes the run and, when the run ends in a `//`,
    /// ends the line at this node's indent — the host token that follows then opens the
    /// next line, shedding the one separator space it would have had after the value.
    /// With nothing pending it is inert.
    ///
    /// Fits: with a suffix pending it is the hard line the flush will render, so a group
    /// measured FLAT across it does not fit (the group enclosing the island breaks, as a
    /// `//` the author wrote there breaks it), while a group that closes BEFORE it — the
    /// value's own groups — measures to it with the pending suffix's width charged, and
    /// fits if that does: the line ends here. That second half is where it parts from
    /// [`LineSuffixBoundary`](DocNode::LineSuffixBoundary), whose look-ahead answer is
    /// "does not fit" and would break every group the deferred run sits in.
    ///
    /// The run's shape is read statically when the node is built
    /// ([`DocArena::pending_run_at_end`]), since the fits walk resolves no text:
    /// `run_width` is what the flush puts on the line it ends, and `run_breaks` whether the
    /// flush holds a `//` at all — a run of blocks alone flushes inline and ends nothing.
    ///
    /// The render records whether a run flushed here under the node's own id, which an
    /// [`DocArena::if_break_with_id`] reads ([`DocArena::embed_end`]).
    EmbedEnd {
        run_width: u16,
        run_breaks: bool,
        dedent_closer: bool,
    },

    /// Force parent group to break
    BreakParent,

    /// Flush-scoped break: force only the nearest enclosing group that can
    /// actually END THE LINE after this point — the group a deferred
    /// [`LineSuffix`](DocNode::LineSuffix) run flushes in.
    ///
    /// Emitted right after a deferred trailing comment whose construct is
    /// stripped from the output (a redundant paren shell): the comment must
    /// meet a line end, but [`BreakParent`](DocNode::BreakParent) would force
    /// *every* enclosing group — including intermediate groups with no line
    /// opportunity after the suffix, whose break the reparse cannot reproduce
    /// (the comment lands past their closer), a format∘format ≠ format class.
    ///
    /// Semantics live in `arena_fits`: walking this node sets a pending-flush
    /// state under which a *flat* breakable line — a `Line(Normal|Soft)` or an
    /// `IfBreak` whose break arm can break — does not fit, so the group owning
    /// the next line opportunity breaks and the flush lands there, while a
    /// group with no line after the suffix stays flat. Invisible to
    /// `will_break` (it forces no particular group) and a no-op at render.
    FlushBreak,

    /// Flow-probe completion sentinel. Never built by a printer: the renderer pushes it
    /// behind the subtree of a node whose context carries
    /// [`DocContext::flow_break_probe`], and when it pops the arena records whether that
    /// subtree's output contained a newline — the answer
    /// [`DocContext::hold_line_after_broken_flow`] reads at the immediately following
    /// fill. Zero-width and skipped by every measurement walk; invisible to `will_break`.
    FlowProbeEnd,

    /// A conditional-group state guarded by a probe line.
    ///
    /// Meaningful only as a state of a conditional group — a non-final one,
    /// except a [`GateAdmit::Never`] state, which is final. At state selection
    /// the renderer first measures `probe` flat on a hypothetical fresh line one
    /// indent level deeper than the group, completed by the group's rest
    /// commands, and `admit` reads that answer:
    ///
    /// - [`GateAdmit::ProbeOverflows`] — the member-chain flat-object hug window.
    ///   If the probe fits, the state is skipped entirely: the layout the probe
    ///   stands for (the expanded member chain keeping its last argument flat on
    ///   the continuation line) is the settled form, and admitting this state
    ///   would steal it.
    /// - [`GateAdmit::ProbeFits`] — a state that breaks onto that very line (a
    ///   value hung after an operator), which no fits walk can measure past the
    ///   break: the probe stands in for the line the state puts its value on.
    /// - [`GateAdmit::Never`] — no probe is measured and the state is never
    ///   selected: it is the group's FINAL state, which an enclosing look-ahead
    ///   reads (a break-mode fits walk takes a conditional group's last state)
    ///   but the render never prints — when no earlier state fits, the render
    ///   takes the state before it in break mode instead. For a group whose
    ///   look-ahead form and last-resort form differ (an unbreakable `<…>` to the
    ///   layout around it that still breaks inside past print width).
    ///
    /// An admitted state's `contents` is measured and rendered like any other
    /// state. Everywhere else — fits walks, `will_break`, rebuilds — the node is
    /// transparent to `contents`; `probe` is measured only, never rendered. Its
    /// nodes are mostly shared with the group's other states, but a probe may
    /// hold docs built for it alone (a [`DocArena::line_probe`]'s padding and
    /// hard line, a prefix of the state's line), which no walk but the admission
    /// measure visits; a rebuild carries them along unchanged.
    GatedState {
        probe: DocId,
        contents: DocId,
        admit: GateAdmit,
    },
}

/// Which answer of a [`DocNode::GatedState`]'s probe admits the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateAdmit {
    /// Admitted only while the probe cannot fit on the fresh line.
    ProbeOverflows,
    /// Admitted only while the probe fits on the fresh line.
    ProbeFits,
    /// Never admitted: a final state for enclosing measurement alone, whose
    /// no-fit render falls back to the state before it.
    Never,
}

// `DocNode` must stay free of drop glue: dynamically-built text lives in the
// arena text pool ([`DocText::Pooled`], `MultilineText`), never in per-node
// owned `String`s, so `DocArena::reset()`'s `clear()` and the arena's drop
// free the node store without walking every node to run destructors on the
// few that would carry a payload. A heap-owning variant would silently
// introduce that walk on every reset across all surfaces (CLI workers,
// FFI/N-API/WASM thread-local reuse) — this guard makes it a compile error;
// route the payload through the pool instead.
const _: () = assert!(!std::mem::needs_drop::<DocNode>());

// `DocNode` is an AoS node whose size the whole memory strategy is load-bearing on:
// the arena's node store is walked linearly at render, so the AoS layout's cache locality is
// the point (SoA and per-variant boxing both measured worse), and shrinking the node has been
// refuted repeatedly (a smaller node loses on this traversal-bound engine — the bumpalo lesson).
// A variant that bloats it would silently regress that locality with no other signal, so pin the
// size — a change here is a deliberate decision, not an accident. The size is pointer-width
// dependent (the `AlignRoot { n: usize }` and `DocText::Static(&str)` fat-pointer payloads), so it is
// pinned per target: 24 B on 64-bit (the native flagship, here, in the test build) and 16 B on
// wasm32 (the shipped WASM bundles, where the locality/allocator budget matters most; pinned in
// `tsv_wasm`, which `build:packages` compiles for wasm32).
//
// ⚠️ The drivers are **`Text`** on 64-bit (a `DocText` is 24 B, from `Static`'s fat pointer plus
// its width slot) and **`Group`** on wasm32 (16 B). `WithContext` is *not* one on either target
// (8 B, so 16 B of slack on 64-bit and 8 B on wasm32) — but it is the variant most likely to grow,
// since it carries a `DocContext` by value and a field added there lands on *every* node in the
// store. That is why `DocContext::trailing_reserve` is a `u16` (it is a column count; as a `usize`
// it alone held the whole node store at 32 B), why its layout flags are one packed `u16`, and why
// `DocContext` carries its own size assert with the exact threshold. Check that one first when
// this pin moves.
//
// ⚠️ **The packing that buys 24 B is also charged at every `match` over a `DocNode`, and that
// cost is invisible to a profile board.** `Text` is the niche-carrying variant, so `DocText`'s
// four sub-tags own `DocNode` discriminant values 0..=3 — which means a switch over the node
// kind cannot index its jump table until it has folded those four back into one arm, four ALU
// ops per visit (`lea`/`cmp`/`mov`/`cmovb`). It hides inside the one source line every board
// attributes to "the dispatch", so only a disassembly shows it. `render_doc_iterative` and
// `arena_fits_with_lookahead` both peel it off with an `if let` ahead of the dispatch, where the
// `Text` test IS the fold; see the comments there for the measurements, and for why the same
// peel is a REGRESSION in `subtree_layout_fill`, whose commonest kind sits ABOVE the fold. The
// rule is that a peel pays only where the peeled kind is the fold's own range.
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(size_of::<DocNode>() == 24);

/// The render-time indentation state of a command — Prettier's `Indent`
/// (`document/printer/indent.js`) specialized to tsv's usage.
///
/// Under `useTabs`, a whole indent level is a tab, while a sub-tab `align(n)`
/// offset is `n` literal spaces — so alignment stays tab-width independent (a
/// closing delimiter sits under its opener at *any* tab width). The one subtlety
/// Prettier's `generateIndent` encodes: an `align(n)` that has a further
/// [`Indent`](DocNode::Indent) stacked on top of it rounds up to a whole tab
/// (its spaces are discarded, one tab takes their place); only a *trailing*
/// align run — the state at a line that ends the aligned region, i.e. a closing
/// delimiter — renders as literal spaces. `pending_aligns` tracks that pending
/// run so the round-up is exact.
///
/// The three counts are packed into **one `u32`, using 31 of its bits** — 12 for
/// the tab depth, 7 for the pending-align count, 12 for the align columns — and
/// every update saturates at its field's width rather than wrapping, so the cap
/// fails safe exactly as the previous all-`u16` spelling did. The caps
/// (4095 / 127 / 4095) sit orders of magnitude above anything a document can
/// reach: the print width is 100 columns and the recursion-depth guard caps
/// nesting far below 4095.
///
/// The packing is what holds [`ArenaCommand`] at **8 bytes**, and that size is
/// the point. `Mode` is one bit and `DocId` is a `u32`, so a 6-byte indent plus
/// a mode byte padded out to a 12-byte command — a third of it alignment, and
/// most of the rest counts that never leave two digits. With the three counts in
/// a `u32` the spare 32nd bit takes the mode and a command is exactly two words,
/// so the render loop's command stack writes ONE 8-byte store per push instead
/// of four stores and a shift, reads ONE load per pop instead of four loads and
/// two stack spills, and scales its index in the addressing mode instead of a
/// `lea ×3`. `instructions:u` **−2.377 / −2.519 / −2.418 / −2.313%** on fuz_app /
/// gro / zzz / fuz_ui, **−2.215%** on pure `.svelte`, **−1.111%** on pure `.css`
/// and **−2.336%** at the product entry point (`tsv format --check --jobs 1`);
/// per-side spread ≤0.022%, min ≈ mean ≈ max throughout, against a
/// `profile --bind` structural control (parse + lower/bind, no doc arena) of
/// **−0.004%**. On the twelve-binary layout group with three pooled replicates:
/// cycles **−1.038%** and wall **−0.987%** against a second-draw null of
/// **+0.104% / +0.183%**. `.text` **−1,968 B**.
///
/// ⚠️ The cycles win is under half the instruction win, which is the shape to
/// expect from a **density** lever and the mirror image of the niche peel above
/// it: what comes off here is retired stores and loads that were mostly L1 hits,
/// not a dependency chain. Grade a density change on `instructions:u` — its
/// cycles number follows, at its own rate.
///
/// The representation is fully encapsulated — the render emitter reads it through
/// [`tabs`](RenderIndent::tabs) / [`trailing_align_spaces`](RenderIndent::trailing_align_spaces)
/// / [`column`](RenderIndent::column), never the raw bits.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderIndent(u32);

/// Width of [`RenderIndent`]'s committed-tab-depth field, in bits.
const INDENT_TABS_BITS: u32 = 12;
/// Width of [`RenderIndent`]'s pending-align-count field, in bits.
const INDENT_ALIGNS_BITS: u32 = 7;
/// Width of [`RenderIndent`]'s align-columns field, in bits.
const INDENT_SPACES_BITS: u32 = 12;
// The shifts and saturation caps the three widths above imply.
const INDENT_TABS_MAX: u32 = (1 << INDENT_TABS_BITS) - 1;
const INDENT_ALIGNS_SHIFT: u32 = INDENT_TABS_BITS;
const INDENT_ALIGNS_MAX: u32 = (1 << INDENT_ALIGNS_BITS) - 1;
const INDENT_SPACES_SHIFT: u32 = INDENT_TABS_BITS + INDENT_ALIGNS_BITS;
const INDENT_SPACES_MAX: u32 = (1 << INDENT_SPACES_BITS) - 1;

// The three fields must leave the top bit free: [`ArenaCommand`] stores its
// `Mode` there, which is what makes the command 8 bytes rather than 12.
const _: () = assert!(INDENT_TABS_BITS + INDENT_ALIGNS_BITS + INDENT_SPACES_BITS == 31);

impl std::fmt::Debug for RenderIndent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderIndent")
            .field("tabs", &self.raw_tabs())
            .field("pending_aligns", &self.raw_pending_aligns())
            .field("align_spaces", &self.raw_align_spaces())
            .finish()
    }
}

impl RenderIndent {
    /// The committed tab depth, unwidened.
    #[inline]
    fn raw_tabs(self) -> u32 {
        self.0 & INDENT_TABS_MAX
    }

    /// The pending-align count, unwidened.
    #[inline]
    fn raw_pending_aligns(self) -> u32 {
        (self.0 >> INDENT_ALIGNS_SHIFT) & INDENT_ALIGNS_MAX
    }

    /// The trailing align run's column sum, unwidened.
    #[inline]
    fn raw_align_spaces(self) -> u32 {
        (self.0 >> INDENT_SPACES_SHIFT) & INDENT_SPACES_MAX
    }

    /// Pack three already-capped field values into the bit layout.
    #[inline]
    fn pack(tabs: u32, pending_aligns: u32, align_spaces: u32) -> Self {
        debug_assert!(tabs <= INDENT_TABS_MAX);
        debug_assert!(pending_aligns <= INDENT_ALIGNS_MAX);
        debug_assert!(align_spaces <= INDENT_SPACES_MAX);
        Self(tabs | (pending_aligns << INDENT_ALIGNS_SHIFT) | (align_spaces << INDENT_SPACES_SHIFT))
    }

    /// A pure whole-tab indent at `level`, with no pending sub-tab alignment.
    #[inline]
    pub fn level(level: usize) -> Self {
        Self::pack(level.min(INDENT_TABS_MAX as usize) as u32, 0, 0)
    }

    /// Push one indent level (Prettier's `makeIndent`). Any pending align run is
    /// flushed to whole tabs first — one tab per `align` in the run, its spaces
    /// discarded — then this level adds its own tab.
    #[inline]
    pub(super) fn indented(self) -> Self {
        Self::pack(
            (self.raw_tabs() + self.raw_pending_aligns() + 1).min(INDENT_TABS_MAX),
            0,
            0,
        )
    }

    /// Pop one indent level, purely on whole tabs.
    ///
    /// tsv never dedents across a pending align run, so `pending_aligns` is
    /// always 0 here and the debug assert is a tripwire, not a live branch. This
    /// is **structural, not incidental**: [`Align`](DocNode::Align) offsets are
    /// emitted only by the TS union/intersection member printers, [`Dedent`]
    /// nodes only by the Svelte element printers, and embedded TS renders under a
    /// fresh command context — so the two node kinds never share a render stack.
    /// (A Prettier-faithful dedent would be `queue.slice(0, -1)`; while the
    /// invariant holds, popping a whole tab is exactly that.)
    ///
    /// [`Dedent`]: DocNode::Dedent
    #[inline]
    pub(super) fn dedented(self) -> Self {
        debug_assert_eq!(
            self.raw_pending_aligns(),
            0,
            "dedent across a sub-tab align run is unsupported"
        );
        Self::pack(
            self.raw_tabs().saturating_sub(1),
            self.raw_pending_aligns(),
            self.raw_align_spaces(),
        )
    }

    /// Add a sub-tab `align(n)` offset (Prettier's numeric `makeAlign`): extend
    /// the trailing align run by `n` columns of spaces.
    #[inline]
    pub(super) fn aligned(self, n: u32) -> Self {
        // The count increments from a value the layout already caps, so a plain
        // `+ 1` cannot overflow; `n` is caller-supplied and unbounded, so the
        // column sum needs `saturating_add` BEFORE the cap or it would wrap past
        // it. The asymmetry is load-bearing — do not unify the two spellings.
        Self::pack(
            self.raw_tabs(),
            (self.raw_pending_aligns() + 1).min(INDENT_ALIGNS_MAX),
            self.raw_align_spaces()
                .saturating_add(n)
                .min(INDENT_SPACES_MAX),
        )
    }

    /// Set an absolute whole-tab level (Prettier's `align` root reset — tsv's
    /// [`AlignRoot`](DocNode::AlignRoot) node, used only as level 0 in template
    /// literals), clearing any pending align run.
    #[inline]
    pub(super) fn reset_to_level(self, level: usize) -> Self {
        Self::level(level)
    }

    /// Committed whole indent levels (each one tab wide).
    #[inline]
    pub fn tabs(self) -> usize {
        self.raw_tabs() as usize
    }

    /// Visual column at the start of a line at this indent (tabs at `tab_width`
    /// columns each, plus the trailing align spaces).
    #[inline]
    pub fn column(self, tab_width: usize) -> usize {
        self.raw_tabs() as usize * tab_width + self.raw_align_spaces() as usize
    }

    /// The trailing sub-tab alignment, in literal spaces (written after the
    /// whole tabs by the render indentation emitter).
    #[inline]
    pub fn trailing_align_spaces(self) -> usize {
        self.raw_align_spaces() as usize
    }
}

/// A command in the printer's command stack.
///
/// Holds a `DocId` index, making it `Copy` with no lifetime parameter.
///
/// **Two words, and deliberately so.** The indent's three counts occupy 31 bits
/// of [`RenderIndent`] (see its doc), leaving the 32nd for the one bit a [`Mode`]
/// carries — so a command is `{u32, u32}` and the hot stack moves it with a
/// single load or store. The mode and indent are read back through
/// [`mode`](Self::mode) and [`indent`](Self::indent); only `doc`, which every
/// dispatch reads first, stays a plain field.
#[derive(Clone, Copy)]
pub struct ArenaCommand {
    /// [`RenderIndent`]'s 31 packed bits, with `Mode::Break` in bit 31.
    indent_mode: u32,
    pub doc: DocId,
}

/// Bit 31 of [`ArenaCommand::indent_mode`]: set for [`Mode::Break`].
const COMMAND_MODE_BREAK: u32 = 1 << 31;

// The hot [`CmdStack`] is pushed and popped around once per rendered doc node —
// millions of times per corpus — so the per-command size is the render loop's
// memory traffic, one load and one store each. Eight bytes is a single aligned
// word: widen it and every push, pop and look-ahead read costs two. All fields
// are fixed-width (no pointers), so the size is target-independent, and the
// packed [`RenderIndent`] plus the mode bit holds to 8 on native and wasm32.
#[cfg(test)]
const _: () = assert!(size_of::<ArenaCommand>() == 8);

// `indent_mode` is printed as the two logical values it packs, so the derived
// lint's "field is unused" is exactly wrong here: nothing about the value is hidden.
#[expect(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for ArenaCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArenaCommand")
            .field("indent", &self.indent())
            .field("mode", &self.mode())
            .field("doc", &self.doc)
            .finish()
    }
}

impl ArenaCommand {
    /// Assemble a command from its three logical parts.
    #[inline]
    pub fn new(indent: RenderIndent, mode: Mode, doc: DocId) -> Self {
        Self {
            indent_mode: indent.0 | Self::mode_bit(mode),
            doc,
        }
    }

    /// The mode's packed bit.
    #[inline]
    fn mode_bit(mode: Mode) -> u32 {
        match mode {
            Mode::Flat => 0,
            Mode::Break => COMMAND_MODE_BREAK,
        }
    }

    /// This command's render indentation state.
    #[inline]
    pub fn indent(&self) -> RenderIndent {
        RenderIndent(self.indent_mode & !COMMAND_MODE_BREAK)
    }

    /// This command's layout mode.
    #[inline]
    pub fn mode(&self) -> Mode {
        if self.indent_mode & COMMAND_MODE_BREAK == 0 {
            Mode::Flat
        } else {
            Mode::Break
        }
    }

    /// Create a command with the same context but a different doc.
    #[inline]
    pub fn with_doc(&self, doc: DocId) -> Self {
        Self { doc, ..*self }
    }

    /// Replace the indent, keeping the mode.
    #[inline]
    fn with_indent(self, indent: RenderIndent, doc: DocId) -> Self {
        Self {
            indent_mode: indent.0 | (self.indent_mode & COMMAND_MODE_BREAK),
            doc,
        }
    }

    /// Create a command with one more indent level.
    #[inline]
    pub fn indented(&self, doc: DocId) -> Self {
        self.with_indent(self.indent().indented(), doc)
    }

    /// Create a command with one fewer indent level.
    #[inline]
    pub fn dedented(&self, doc: DocId) -> Self {
        self.with_indent(self.indent().dedented(), doc)
    }

    /// Create a command with an added sub-tab `align(n)` offset.
    #[inline]
    pub fn aligned(&self, n: u32, doc: DocId) -> Self {
        self.with_indent(self.indent().aligned(n), doc)
    }

    /// Create a command with the indent reset to an absolute whole-tab level
    /// (Prettier's align root reset), clearing any sub-tab alignment. Mirrors
    /// [`RenderIndent::reset_to_level`].
    #[inline]
    pub fn reset_to_level(&self, level: usize, doc: DocId) -> Self {
        self.with_indent(self.indent().reset_to_level(level), doc)
    }

    /// Create a command with a specific mode.
    #[inline]
    pub fn with_mode(&self, mode: Mode, doc: DocId) -> Self {
        Self {
            indent_mode: (self.indent_mode & !COMMAND_MODE_BREAK) | Self::mode_bit(mode),
            doc,
        }
    }
}

/// Render work-list — a plain `Vec`, and the parked-allocation policy that is
/// what makes a plain `Vec` affordable.
///
/// ⭐ **A `SmallVec` here cost ~1% of a format run.** Its inline-or-spilled union
/// is re-selected on every `push` and every `pop` — a capacity load, a spill
/// test, a second data-pointer source — and this stack is pushed and popped
/// around once per rendered doc node, millions of times per corpus. The
/// top-level stack is always spilled after warmup, so that test never once
/// answered "inline" there. Dropping to a `Vec` measured `instructions:u`
/// **−0.917 / −0.973 / −0.980 / −1.041%** on four app corpora, **−0.925%** on
/// pure Svelte, **−0.598%** on pure CSS and **−0.955%** at the product entry
/// point.
///
/// What the `SmallVec` bought was the *sub*-renders: the renderers run many
/// times per file (CSS per declaration/value, Svelte per template expression),
/// each from empty, and the inline capacity kept those off the heap. A `Vec`
/// grown from nothing at each of them hands most of the win straight back — a
/// fuz_ui pass reads **−0.078%** unparked against **−0.917%** parked — so every
/// render takes a warm allocation instead of building one: the top-level render
/// borrows [`DocArena::borrow_top_render_stack`], and the nested renders
/// take [`DocArena::take_sub_render_stack`].
///
/// The fits walk's own `SmallVec<[(DocId, Mode); 16]>` is deliberately NOT the
/// same call: it is spun up per *call* rather than per node, never spills, and
/// parking it the same way measured **+0.12 / +0.27%**.
pub(super) type CmdStack = Vec<ArenaCommand>;

/// Inline-backed pending `line_suffix` buffer. Line suffixes are sparse — a flush
/// carries one in the overwhelming majority of cases, and two where a construct's
/// deferred comment meets a statement trailer (the run
/// `doc::arena_render::flush_line_suffix` must separate) — so `N = 4` is generous
/// headroom at a 64-byte inline footprint, keeping even the rare suffix push off the heap.
pub(super) type LineSuffixBuf = SmallVec<[ArenaCommand; 4]>;

/// Cell values for the per-node **subtree-layout cache** — the one memo behind
/// both layout questions a doc subtree answers: *does it force a break*
/// ([`DocArena::will_break`], asked at BUILD from the printers) and *how wide is
/// it flat* (`arena_fits`'s fast path, asked at RENDER). One `u32` carries both,
/// because the two answers are not independent: **a forced break implies no flat
/// width**, by structural induction over every node kind — each arm of
/// [`DocArena::subtree_layout_fill`] either reports no forced break, or reports
/// one together with an absent flat width. So a cell is exactly one of
///
/// - [`LAYOUT_UNKNOWN`] — not computed yet;
/// - [`LAYOUT_BREAKS_FORCED`] — no flat width, and `will_break` is **true**;
/// - [`LAYOUT_BREAKS_SOFT`] — no flat width, `will_break` **false**: a line
///   suffix, a suffix boundary, a flush-scoped break, an `if_break` whose flat
///   arm breaks, a format-ignored verbatim slice. Nodes the fits walk must
///   *visit* rather than summarize, and that force no break on their group;
/// - anything `<=` [`LAYOUT_WIDTH_MAX`] — a break-free flat width, `will_break`
///   false.
///
/// The ordering is load-bearing: the fits fast path's common case is one
/// unsigned compare (`v <= LAYOUT_WIDTH_MAX`), and `will_break`'s is
/// `v == LAYOUT_BREAKS_FORCED` once "computed" is established.
///
/// Packing as `u32` (vs an 8-byte enum) halves the footprint — one `u32` per doc
/// node, ~4 nodes per source byte — which matters most for the memory-constrained
/// WASM target. (A further `u16` narrowing was measured and rejected:
/// instructions +0.26% on the fits-memo path, and WASM steady high-water +2 pages
/// — the halved realloc-size sequence fragments under talc's binning.)
///
/// ⚠️ **A summed width is clamped to [`LAYOUT_WIDTH_MAX`], and here that is a
/// CORRECTNESS requirement, not a nicety.** In the two-sentinel flat-width cache
/// this replaces, a width aliasing a sentinel merely deferred the node to the
/// walk or recomputed it — both benign. Now an alias of [`LAYOUT_BREAKS_FORCED`]
/// would make `will_break` answer **true** for a subtree that does not break,
/// which is wrong output. Reaching it needs a ~4 GB-wide break-free flat subtree,
/// so the clamp costs nothing in practice: one `min` per *summing* node, never
/// per child (`saturating_add` is monotone, so clamping the finished sum gives
/// the same answer as clamping every addend). Leaf widths cannot reach it at all
/// — they come from the `u16` text-width slot.
pub(super) const LAYOUT_UNKNOWN: u32 = u32::MAX;
pub(super) const LAYOUT_BREAKS_FORCED: u32 = u32::MAX - 1;
pub(super) const LAYOUT_BREAKS_SOFT: u32 = u32::MAX - 2;
pub(super) const LAYOUT_WIDTH_MAX: u32 = u32::MAX - 3;

/// The packed layout cell of a leaf `Text`, read straight off the node.
///
/// The one node kind whose layout answer needs no traversal, no children and no
/// cache lookup beyond its own slot — which is why
/// [`DocArena::subtree_layout_memo`] answers it inline rather than calling into
/// [`DocArena::subtree_layout_fill`] for it.
///
/// ⚠️ **Only that probe does.** `arena_fits`'s `flat_width_memo` still calls in,
/// which is why the fill keeps a `Text` arm of its own; both route through here
/// so the rule below is stated once. Whether the fits probe should answer it too
/// is unmeasured — 98.9% of its probes are already warm, so its miss path is
/// thin.
///
/// ⚠️ **A format-ignored JavaScript-level verbatim slice is layout-opaque, and
/// that is the opposite verdict from every other newline-bearing text.**
/// prettier's `printIgnored` string is invisible to `willBreak`, so it reports
/// [`LAYOUT_BREAKS_SOFT`] — no single-line width, no forced break — where a
/// line-continuation string, or a frozen Svelte template node (which
/// prettier-plugin-svelte joins with `literalline`), reports
/// [`LAYOUT_BREAKS_FORCED`]. The fits walk still sees the newline through the
/// same width slot.
///
/// ⚠️ **Reading the width slot raw for the verbatim case is deliberate, and it
/// is the third spelling tried.** All three were built and measured on fuz_app
/// (16 execs a side, per-side spread 0.002%, so the gaps are real):
///
/// | shape | Δ instr | `tsv` `.text` |
/// | --- | --- | --- |
/// | this one — raw slot here, `cached_width()` for the rest | 0 | 2,895,621 |
/// | one `Text` arm, `matches!(VerbatimSpan)` on the newline path | +0.023% | 2,895,605 |
/// | this case routed through `cached_width()` too | +0.070% | 2,895,445 |
///
/// The single-seam shapes are the tidier ones and both cost instructions on the
/// hottest fill on the board, so the duplicate decode stays — it is one test
/// of the ONE flag this type has, and the policy that keeps it at one is on
/// [`pooled_text_width`].
#[inline]
fn text_subtree_layout(t: &DocText) -> u32 {
    match t {
        DocText::VerbatimSpan(_, w) => {
            if *w & TEXT_WIDTH_NEWLINE_FLAG == 0 {
                u32::from(*w)
            } else {
                LAYOUT_BREAKS_SOFT
            }
        }
        // A newline-bearing Text (a line-continuation string) breaks the
        // enclosing group, like `MultilineText` — the width cache flags it via
        // `HasNewline`, and under the eager `pooled_text_width` policy that flag
        // is always set at build, so this reads the answer rather than
        // approximating it.
        _ => match t.cached_width() {
            CachedWidth::Width(w) => u32::from(w),
            CachedWidth::HasNewline { .. } => LAYOUT_BREAKS_FORCED,
        },
    }
}

/// The layout answer for a `Line`, as both halves of the fused walk read it.
///
/// Companion to [`text_subtree_layout`]: one emitter for a kind
/// [`DocArena::subtree_layout_memo`] answers inline and
/// [`DocArena::subtree_layout_fill`] still reaches from the fits walk's own
/// probe.
#[inline]
fn line_subtree_layout(kind: LineKind) -> u32 {
    match kind {
        LineKind::Hard | LineKind::Literal => LAYOUT_BREAKS_FORCED,
        LineKind::Soft => 0,
        LineKind::Normal => 1,
    }
}

/// Downgrade a forced break to a soft one, keeping every other cell value.
///
/// The one place the two fused questions diverge: an `if_break`'s flat width is
/// its flat arm's, but its forced-break verdict is `false` whatever that arm
/// holds — so a [`LAYOUT_BREAKS_FORCED`] coming up from `flat_doc` must lose the
/// break half and keep the "no flat width" half.
#[inline]
fn soften_forced_break(v: u32) -> u32 {
    if v == LAYOUT_BREAKS_FORCED {
        LAYOUT_BREAKS_SOFT
    } else {
        v
    }
}

/// A measured width, clamped below [`TEXT_WIDTH_NEWLINE_FLAG`] — the one spelling
/// of the clamp every width producer goes through, for a single-line width and
/// for the first-line width a newline-bearing text carries under the flag alike.
///
/// Unlike the `u32` flat-width cache above (where aliasing needs a ~4 GB subtree
/// and is benign anyway), a `u16` alias is **reachable** — a single-line
/// non-ASCII text of 32,768 columns — and `as u16` alone would be wrong twice
/// over: 32,768 sets [`TEXT_WIDTH_NEWLINE_FLAG`] (fits would treat the line as
/// ending inside the text, after charging a first line of zero) and 65,536 or
/// more wraps (a huge text cached as narrow → "always fits").
///
/// Clamping is verdict-preserving: every fits comparison is against a print width
/// orders of magnitude below the clamp, so "32,767" and the true width answer
/// identically. The same holds for the other consumer, `render_text`'s column
/// advance — the column only feeds threshold comparisons (the print width) far
/// below the clamp, and resets at each newline.
#[inline]
const fn clamp_text_width(w: usize) -> u16 {
    if w >= TEXT_WIDTH_FIRST_LINE_MASK as usize {
        TEXT_WIDTH_FIRST_LINE_MASK
    } else {
        w as u16
    }
}

/// The slot of a newline-bearing text: [`TEXT_WIDTH_NEWLINE_FLAG`] over its
/// first line's clamped width.
#[inline]
const fn newline_text_width(first_line_width: usize) -> u16 {
    TEXT_WIDTH_NEWLINE_FLAG | clamp_text_width(first_line_width)
}

/// The eager width-cache policy for doc text, and it has **no exceptions**:
/// pool-stored text ([`DocText::Pooled`], `MultilineText` first lines),
/// verbatim source slices ([`DocArena::source_span`], identifier names
/// included), and `text()` statics (amortized through the arena's static cache
/// — measured once per unique string, not per node) **always** cache a real
/// width or the flagged first-line width at build. So every width query (the fits
/// walk, `render_text`'s column advance) answers from the node alone, the fits
/// path never borrows the pool, and render's per-text byte scan is skipped. A
/// newline-bearing text caches its **first** line's width under the flag
/// ([`newline_text_width`]) — the one thing the fits walk charges of it.
///
/// ⭐ Identifier names look like the obvious exception — high-frequency and
/// newline-free — and they are the **wrong** one: a name span is ~15% of all
/// doc nodes, and deferring its width did not avoid the scan, it only moved it
/// into the two hottest functions on the board — `render_text`'s column advance
/// (which every emitted name reaches) and the fits path's own on-demand measure.
/// Measuring eagerly instead, in a small builder function, costs one scan and
/// retires both: `instructions:u` −1.20 / −1.14 / −1.11 / −1.02% on TS-heavy
/// corpora and −0.81% on pure Svelte, with pure CSS — which emits no name spans
/// — an exact **−0.000%** null, and `.text` −1,136 B. ⚠️ The ~+1.1% price tag a
/// name deferral used to carry never described this policy: it was measured over
/// a *bundle* — names together with `text()` statics and the since-deleted
/// interner's symbols — before the static cache
/// amortized one half of it and before both the build-time and on-demand
/// measures were unified onto this one. Re-take a width-policy
/// number against the current pair of measures; do not inherit one.
///
/// ⭐ With no exception left the policy is **total**, and that is a structural
/// property, not just a fast one: a width query cannot need the document source,
/// so the whole fits walk answers from `nodes` + the width slot. The deferral
/// mechanism it retired — a `TEXT_WIDTH_NOT_COMPUTED` sentinel, an on-demand
/// measure in `arena_fits`'s `text_flat_width`, a re-scan arm in `render_text` —
/// is deleted rather than kept unreached; re-introducing a deferral means
/// re-introducing the `source` threading it forces on every fits caller.
///
/// The measured width is clamped below the flag by
/// [`clamp_text_width`], which carries that argument.
///
/// ⚠️ **This is the SLICE form, and it is now the minority one.** A text that is
/// a span of the document goes through [`source_span_width`] instead — 98% of the
/// width measures in a format run — because a span can read its host's words where
/// a slice has none to read. What follows is the argument both share; the span
/// form's own doc carries what is different about it.
///
/// ⭐ **The width of a plain ASCII line IS its byte count, so the measure is a
/// SEARCH, not a sum.** One [`crate::printing::next_width_relevant`] pass asks
/// for the first byte that could make the answer anything else — a `\t`, a `\n`,
/// or a non-ASCII byte — and finding none *is* the measurement: `s.len()`, with
/// nothing accumulated along the way. That is what retired the three shapes this
/// function used to hold at once (a per-byte width sum under a length gate, then
/// `contains('\n')` + `is_ascii` + a tab count, three passes over the same
/// bytes): the sum cost **13 instructions a byte** and every caller of it threw
/// the sum away, and the searcher trio cost about ten more per byte on top of
/// an out-of-line call. The scan is **~1.9 instructions a byte** and 98.97% of
/// slices never leave it.
///
/// ⚠️ The `\t` / `\n` / non-ASCII class is not decoration and is not free to
/// narrow: drop `\t` and every tabbed text measures one column short per tab,
/// which changes a fits verdict and nothing else — invisible to the fixtures and
/// to any size of format or wire diff. The class is spelled once
/// (`printing::is_width_relevant`) for the word loop's tail, and
/// [`pooled_text_width_cold`] re-asks it per hit.
///
/// Answers identically to probing `contains('\n')` and then
/// [`crate::printing::visual_width`]: on an all-ASCII slice the width is
/// `len + tabs * (TAB_WIDTH - 1)`, which is exactly that function's ASCII fast
/// path; a `\n` anywhere yields the flag the `contains` probe would have, over
/// the width of the text before it; and the first non-ASCII byte hands the
/// **whole** slice to the grapheme walk, with a `\n` sitting *after* that byte
/// still found first.
///
/// ⚠️ It mirrors that **ASCII fast path**, where a control character counts as
/// one column — deliberately *not* `printing::ascii_char_width`, which counts it
/// as zero and which only the grapheme-walking path uses (see
/// `visual_width_mixed`). The two disagree on purpose; a walk that reached
/// for the "obvious" shared helper would silently change every width holding a
/// control byte. The exhaustive equivalence test grades this arm with `\x00`,
/// `\x1b` and `\x7f` precisely because no corpus does.
#[inline]
fn pooled_text_width(s: &str) -> u16 {
    let bytes = s.as_bytes();
    let first = next_width_relevant(bytes, 0);
    if first == bytes.len() {
        return clamp_text_width(bytes.len());
    }
    pooled_text_width_cold(s, first)
}

/// [`pooled_text_width`] for a span of `source` — the form 98% of the width measures in a
/// format run take, and the one that can answer a short text in a single word test.
///
/// ⭐ **A span is a region of a much longer buffer, so the scan has eight bytes to read
/// wherever the span itself does not.** [`pooled_text_width`] hands its scan a *slice*,
/// where a text under eight bytes has no word to form and falls to a scalar class test at
/// **9 instructions a byte** — and 61% of the spans measured here are that short (390,799
/// of 640,428 on a 1,666-file TypeScript corpus; only **72** of them sit within eight
/// bytes of the document's end, where a word genuinely is not readable).
/// [`next_width_relevant_in`] reads the host's word and ignores what falls past the span.
///
/// ⭐ **And the width question wants BYTES, not a `&str`.** `Span::extract` is a `str`
/// index, so it pays [`str::is_char_boundary`] at both ends — two loads and four compares
/// the measurement never asked for — and the slice it returns is then read as
/// `as_bytes()` anyway. The `&str` is materialized only on the cold arm, which needs it
/// for the grapheme walk.
///
/// ⚠️ **That does not weaken the boundary check on any span with content in it**, because
/// a non-empty span that splits a multi-byte character always *contains* a byte at or
/// above `0x80` and so always reaches the cold arm: if `end` is not a boundary the
/// character's lead byte lies at or after `start`, and if `start` is not a boundary it is
/// itself a continuation byte; either way the class scan hits it and the cold arm's
/// `span.extract` panics exactly where it always did. The one case that changes is a
/// **degenerate empty span at a non-boundary offset**, which now measures 0 instead of
/// panicking here — and still panics at render, where `resolve_text` re-slices the same
/// span out of the same document.
///
/// ⚠️ **`#[inline(always)]` is load-bearing and is a MEASURED choice, not a hint.** Left
/// to itself LLVM outlines this and folds `source_span`'s allocation into its 38 callers
/// — the opposite split from the one the `str`-slicing shape got, where measure and
/// allocation were one outlined function. That split costs **+0.301%** of a TypeScript
/// format run (~10 instructions of call and marshalling per span, measured as its own
/// variant), which is more than the scan it was paying for.
///
/// ⚠️ Both attributes here and on [`DocArena::source_span`] describe a **threshold**, so a
/// toolchain bump can move them: re-check that `source_span` is still one outlined symbol
/// (`objdump -C | grep 'DocArena::source_span>:'`) before trusting a width-path number
/// taken after one.
#[expect(clippy::inline_always)]
#[inline(always)]
fn source_span_width(span: Span, source: &str) -> u16 {
    let bytes = source.as_bytes();
    let start = span.start as usize;
    let end = span.end as usize;
    if start > end || end > bytes.len() {
        // A malformed span: hand it to the `str` index so it fails where it always did.
        return source_span_width_invalid(span, source);
    }
    let first = next_width_relevant_in(bytes, start, end);
    if first == end {
        return clamp_text_width(end - start);
    }
    source_span_width_cold(span, source, first - start)
}

/// The arm [`source_span_width`]'s scan hands a span to once it finds a byte the width
/// depends on — **1.0%** of the spans a format run measures. It is the first point that
/// needs the text as a `str`, so this is where the slice (and its char-boundary check)
/// is taken.
#[cold]
#[inline(never)]
fn source_span_width_cold(span: Span, source: &str, first: usize) -> u16 {
    pooled_text_width_cold(span.extract(source), first)
}

/// A span outside its document, or inverted — where `span.extract`'s `str` index is
/// what raises the panic, before the width is ever asked for. Spelled as a delegation
/// rather than an `unreachable!` so it stays correct if the guard above is ever
/// loosened, and outlined so the arm never widens [`source_span_width`] past what LLVM
/// will fold back into its caller (the width measure and the node allocation are one
/// function in the emitted code, as they were when the measure took the `str` slice).
#[cold]
#[inline(never)]
fn source_span_width_invalid(span: Span, source: &str) -> u16 {
    pooled_text_width(span.extract(source))
}

/// The arm [`pooled_text_width`]'s scan hands a slice to once it finds a byte the
/// width depends on — a `\t`, a `\n`, or a non-ASCII one. `first` is that byte's
/// index, so the scan is never restarted from zero.
///
/// **`#[cold]` is right here and was not right for what this replaced.** The arm
/// it grew out of was entered by every slice past a 32-byte length gate — a
/// normal input, and the gate's doc said so — where this one is entered only by a
/// slice that actually holds one of those three bytes: **1.03% of calls** on a
/// 1,666-file TypeScript corpus (6,624 of 642,179 — 1,414 short slices holding a
/// tab, 4,330 holding a non-ASCII byte, 880 long ASCII ones holding a tab), and
/// the `\n` case is **1 call in 642,179**. The premise the old refusal rested on is gone with the gate.
///
/// The tab arm keeps scanning rather than falling to a count: only 2,294 of the
/// 642,179 calls hold a tab at all (0.36%), so the loop it re-enters is expected
/// to run once.
/// The non-ASCII arm must still answer the newline question first — a `\n` after
/// the first non-ASCII byte outranks the width — and it hands
/// `visual_width_mixed` the **whole** slice (or, under a newline, the whole first
/// line), since a grapheme cluster can begin on the ASCII byte before the one
/// that fired.
///
/// A newline found by the `\n` arm has only ASCII and tabs before it (every
/// non-ASCII byte is width-relevant and would have been reached first), so the
/// first line's width is its byte count plus the tabs counted so far.
#[cold]
#[inline(never)]
fn pooled_text_width_cold(s: &str, first: usize) -> u16 {
    let bytes = s.as_bytes();
    let mut i = first;
    let mut tabs = 0usize;
    loop {
        match bytes[i] {
            b'\n' => return newline_text_width(i + tabs * (TAB_WIDTH - 1)),
            b'\t' => tabs += 1,
            _ => {
                let lf = next_lf(bytes, i);
                return if lf != bytes.len() {
                    newline_text_width(visual_width_mixed(&s[..lf], TAB_WIDTH))
                } else {
                    clamp_text_width(visual_width_mixed(s, TAB_WIDTH))
                };
            }
        }
        i = next_width_relevant(bytes, i + 1);
        if i == bytes.len() {
            return clamp_text_width(bytes.len() + tabs * (TAB_WIDTH - 1));
        }
    }
}

/// The statics pre-interned at FIXED ids — the prelude every document opens with.
///
/// Seeded into `nodes` by the constructors and by [`DocArena::reset`] ahead of the
/// document's own first node, so `DocId(i)` names `PRELUDE[i]` in every document and
/// [`DocArena::text`] answers a member without touching the arena at all:
/// [`prelude_index`] is a byte match, and at a literal call site (`text(",")`) it
/// folds to a constant — no hash, no slot probe, no generation compare, no node.
/// A runtime argument (`text(op_str)`) pays the match — a length switch and a byte
/// jump table — ahead of the probe it would have paid anyway.
///
/// The set is the census's head, not a style: on a 1,666-file TypeScript corpus
/// one pass makes 1.57 M `text()` calls over 169 distinct strings, and the ten
/// hottest — every one a piece of punctuation — carry 74% of them (the top 24,
/// 90%); a Svelte corpus adds the tag punctuation, a CSS corpus is 98% `(` / `)`
/// / `,` / `: `. Keywords stay out: their population is long-tailed and each
/// entry lengthens the runtime match. Widths are stated, not measured — a test
/// grades them against [`pooled_text_width`] — and `"\t"` is [`TAB_WIDTH`].
///
/// Seeding costs a node per entry per document (a few hundred instructions),
/// and buys back the once-per-document miss each member used to pay.
///
/// The stateless singletons sit AHEAD of the statics, at [`PRELUDE_EMPTY`] through
/// [`PRELUDE_FLOW_PROBE_END`]: a node with no per-use state — [`DocArena::empty`],
/// the four [`LineKind`]s, the line-suffix boundary, the two breaks, the flow-probe
/// sentinel — serves every call site in a document, and at a fixed id every builder
/// is a constant rather than a generation-gated cell probe.
const PRELUDE: &[(&str, u16)] = &[
    ("(", 1),
    (")", 1),
    (" ", 1),
    (";", 1),
    (",", 1),
    (".", 1),
    ("}", 1),
    ("{", 1),
    (":", 1),
    ("\t", 2),
    ("]", 1),
    ("[", 1),
    ("<", 1),
    (">", 1),
    ("=", 1),
    ("!", 1),
    ("?", 1),
    ("`", 1),
    ("&", 1),
    ("*", 1),
    ("+", 1),
    ("-", 1),
    ("/", 1),
    ("|", 1),
    ("~", 1),
    ("\"", 1),
    (": ", 2),
    (", ", 2),
    (" =", 2),
    (" =>", 3),
    (" = ", 3),
    ("</", 2),
    ("=\"", 2),
    ("/>", 2),
    ("{ ", 2),
    (" }", 2),
    ("| ", 2),
    (" (", 2),
    (") ", 2),
    ("${", 2),
    ("?.", 2),
    ("...", 3),
    ("{}", 2),
    ("> ", 2),
    (" &", 2),
    ("=>", 2),
];

/// The static shape of a deferred run still pending at a doc's end
/// ([`DocArena::pending_run_at_end`]), as an [`DocNode::EmbedEnd`] flushes it: every comment
/// behind no `//` glued onto the line (an own-line one loses its break), every comment behind
/// a `//` on a line of its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PendingRun {
    /// Columns the flush puts on the line it starts on — through the first `//`, which
    /// ends that line — saturated one column past [`crate::config::PRINT_WIDTH`]: a fits
    /// question asked within the print width answers as it would for the true width.
    pub width: u16,
    /// The run holds a `//`: flushing it ends a line.
    pub has_line_comment: bool,
    /// The run's last comment is a `//`: the host's closer cannot follow it on its line.
    pub ends_in_line_comment: bool,
    /// No layout can flush the run before the doc's end: no breakable line follows it, in
    /// any `if_break` arm or conditional-group state. `false` — the run reaches the end
    /// only in SOME layouts (a hugged last argument whose list may still break) — is a
    /// verdict only the render can settle.
    pub certain: bool,
}

/// Where the walk stands with respect to a deferred run.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
enum RunQueue {
    /// Nothing is queued, or every queued run met a flush point.
    #[default]
    Empty,
    /// A run is queued and no layout has flushed it since.
    Pending,
    /// A run is queued, and some layout may have flushed it already: a breakable line
    /// passed, or an `if_break` arm or a conditional-group state flushes it.
    MaybeFlushed,
}

/// The shape of the run the walk is reading — a [`PendingRun`] short of `certain`, which
/// is not walk state: the walk's join decides it, from the queue, only at the root.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
struct RunShape {
    /// Columns on the run's first line, saturated at [`RUN_WIDTH_CAP`].
    width: u16,
    has_line_comment: bool,
    ends_in_line_comment: bool,
}

/// Where a run's first-line width saturates, one column past the print width.
///
/// A render's fits walk starts from a remaining width of at most its print width (the
/// render takes the column off it, saturating at zero) and only ever subtracts. So once a
/// run is wider than the print width, the [`DocNode::EmbedEnd`] that charges it leaves the
/// walk negative whatever the true width, and the walk answers "does not fit" either way.
/// Two askers sit outside that argument. One is the flat-measure render, whose width is
/// unbounded: no width a run can reach makes it overflow, capped or not. The other is a
/// fits walk started wider than [`crate::config::PRINT_WIDTH`] — `arena_fits` asked for
/// such a width, or a render under a wider test `RenderConfig` — which no printer does.
///
/// The cap is what bounds [`DocArena::pending_run_at_end`]'s memo per node. Alternatives
/// that queue runs of different widths ahead of a shared subtree reach it in at most this
/// many widths, not one per subset sum of their widths.
const RUN_WIDTH_CAP: u16 = crate::config::PRINT_WIDTH as u16 + 1;

impl RunShape {
    /// Add `columns` to the first line's width.
    fn widen(&mut self, columns: u16) {
        self.width = self.width.saturating_add(columns).min(RUN_WIDTH_CAP);
    }
}

/// The state [`DocArena::pending_run_at_end`]'s walk threads in emission order — and,
/// with the node, its memo key: the derived `Hash` and `Eq` read every field, so a field
/// added here keys the memo without another edit.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
struct SuffixWalk {
    queue: RunQueue,
    /// A [`DocNode::FlushBreak`] was passed: the next soft or normal line breaks.
    flush_armed: bool,
    /// The pending run's shape so far.
    run: RunShape,
    /// The run's first line is complete (a `//` ended it): later widths are not on it.
    width_done: bool,
}

impl SuffixWalk {
    fn pending(self) -> bool {
        self.queue != RunQueue::Empty
    }

    /// Join the outcomes of two alternative layouts from one starting state.
    fn either(a: Self, b: Self) -> Self {
        match (a.pending(), b.pending()) {
            (true, true) if a.queue == RunQueue::Pending && b.queue == RunQueue::Pending => a,
            (true, _) => Self {
                queue: RunQueue::MaybeFlushed,
                ..a
            },
            (false, true) => Self {
                queue: RunQueue::MaybeFlushed,
                ..b
            },
            (false, false) => a,
        }
    }
}

/// What [`DocArena::may_end_on_comment_text`]'s walk has seen last, ordered so that two
/// alternative layouts join by `max`: toward [`Self::Comment`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum LastLeaf {
    /// No printed leaf yet.
    Nothing,
    /// Code.
    Code,
    /// A comment, in at least one layout.
    Comment,
}

/// What one doc does to the [`LastLeaf`] reading in front of it — the walk's per-node
/// transfer, `state ↦ max(found, if passes { state } else { Nothing })`.
///
/// Every node's effect has this form: a printed leaf replaces the state, a node that
/// prints none passes it through, and an alternative's join is `max` of the two results.
/// So the effect is a function of the node alone, and a shared subtree's is read once
/// (the walk's memo), however many parents reach it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LastLeafStep {
    /// The last leaf the doc prints, worst over its layouts.
    found: LastLeaf,
    /// Some layout of the doc prints no leaf: the reading ahead of it shows through.
    passes: bool,
}

impl LastLeafStep {
    /// A doc that prints no leaf.
    const PASS: Self = Self {
        found: LastLeaf::Nothing,
        passes: true,
    };

    /// A doc that always ends on a leaf of class `found`.
    const fn leaf(found: LastLeaf) -> Self {
        Self {
            found,
            passes: false,
        }
    }

    /// `self`, then `next`, in print order.
    fn then(self, next: Self) -> Self {
        Self {
            found: if next.passes {
                next.found.max(self.found)
            } else {
                next.found
            },
            passes: self.passes && next.passes,
        }
    }

    /// Join two alternative layouts read from one starting state.
    fn either(self, other: Self) -> Self {
        Self {
            found: self.found.max(other.found),
            passes: self.passes || other.passes,
        }
    }
}

/// Visits a doc walk outside the renderer takes before it starts its memo.
///
/// The doc is a DAG — a conditional group's states and an `if_break`'s arms share
/// subtrees — so a walk that reads every alternative revisits a shared subtree once per
/// path to it, which nesting compounds exponentially. On real code these walks are
/// short (a handful of nodes a call), so a call starts unmemoized and allocates nothing;
/// one that outlives this many visits memoizes the rest of its walk, which bounds it by
/// the (node, key) pairs it can reach — the nodes alone for a walk keyed on the node, the
/// nodes times the states that reach each for `suffix_walk`, a constant per node
/// ([`RUN_WIDTH_CAP`] widths times the state's flags). Unit tests memoize from the first
/// visit unless they ask for this threshold (`test_memo_after`), so the memo answers
/// every test that reaches a walk.
const WALK_MEMO_AFTER: u32 = 256;

/// The threshold a [`WalkMemo::new`] starts from: [`WALK_MEMO_AFTER`], or under test the
/// one `test_memo_after::with` names (zero by default).
#[cfg(not(test))]
#[inline]
const fn walk_memo_after() -> u32 {
    WALK_MEMO_AFTER
}

#[cfg(test)]
fn walk_memo_after() -> u32 {
    test_memo_after::get()
}

/// The walk-memo threshold unit tests run at: per thread, zero (memoize from the first
/// visit) unless a test sets it to exercise the production switch-over.
#[cfg(test)]
pub(super) mod test_memo_after {
    use std::cell::Cell;

    thread_local! {
        static AFTER: Cell<u32> = const { Cell::new(0) };
    }

    pub(super) fn get() -> u32 {
        AFTER.with(Cell::get)
    }

    /// Run `f` with every walk memo starting after `visits`, then restore the threshold.
    pub(in crate::doc) fn with<R>(visits: u32, f: impl FnOnce() -> R) -> R {
        let prev = AFTER.with(|a| a.replace(visits));
        let out = f();
        AFTER.with(|a| a.set(prev));
        out
    }
}

/// The per-call memo of a doc walk: `key → value` for a walk whose answer at a node is a
/// function of the key alone (the node, plus any state threaded into it). Nodes are
/// immutable once allocated and one call reads one source, so an entry never goes stale
/// within the call that wrote it. Unallocated until the walk has taken
/// [`WALK_MEMO_AFTER`] visits — until then a visit is one counter test, the memo's whole
/// cost on the short calls real code makes — and an answer computed before then is simply
/// not stored.
struct WalkMemo<K, V> {
    /// Visits left before the memo starts; zero once it has.
    unmemoized: u32,
    map: Option<FxHashMap<K, V>>,
}

impl<K: Copy + Eq + std::hash::Hash, V: Copy> WalkMemo<K, V> {
    #[inline]
    fn new() -> Self {
        Self::starting_after(walk_memo_after())
    }

    /// A memo that starts once the walk has taken `visits` visits.
    #[inline]
    const fn starting_after(visits: u32) -> Self {
        Self {
            unmemoized: visits,
            map: None,
        }
    }

    /// Count one visit to `key` and return its memoized value, if any.
    #[inline]
    fn visit(&mut self, key: K) -> Option<V> {
        if self.unmemoized != 0 {
            self.unmemoized -= 1;
            return None;
        }
        self.memoized_get(key)
    }

    /// Store `value` as `key`'s answer (once the memo has started) and return it.
    #[inline]
    fn record(&mut self, key: K, value: V) -> V {
        if self.unmemoized == 0 {
            self.memoized_insert(key, value);
        }
        value
    }

    #[cold]
    #[inline(never)]
    fn memoized_get(&mut self, key: K) -> Option<V> {
        self.map
            .get_or_insert_with(FxHashMap::default)
            .get(&key)
            .copied()
    }

    #[cold]
    #[inline(never)]
    fn memoized_insert(&mut self, key: K, value: V) {
        self.map
            .get_or_insert_with(FxHashMap::default)
            .insert(key, value);
    }
}

/// What a doc can print between where it starts and its first code token — the break
/// half of [`DocArena::leading_comment_break`]'s answer, ordered so the worse layout is the
/// greater.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LeadingBreak {
    /// Nothing ends a line there, in any layout.
    None,
    /// A breakable line ends one in some layout: its group may break, or the line sits in
    /// no group of the doc and takes the enclosing mode.
    Soft,
    /// A line is ended in every layout: a `//`, a hard line, or a breakable line in a group
    /// already forced broken.
    Hard,
}

/// [`DocArena::leading_comment_break`]'s reading of a doc's leading edge — everything it
/// prints ahead of its first code token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeadingEdge {
    /// The worst line break behind a comment there.
    pub brk: LeadingBreak,
    /// The worst line break ahead of any comment there — a line the doc opens with.
    pub bare: LeadingBreak,
    /// A block comment there spans lines: its interior holds a line terminator — any of
    /// ECMAScript's, `<LS>` / `<PS>` included — which is one for ASI by itself (ecma262
    /// sec-comments), whatever follows it.
    pub multiline: bool,
    /// A breakable line behind a comment there sits in no group of the doc, so it takes
    /// the mode of whatever encloses the doc — broken, at a statement.
    pub ungrouped: bool,
    /// The first comment there, when it prints as a verbatim source slice.
    pub first_comment: Option<Span>,
}

impl LeadingEdge {
    /// The edge of a doc that opens on code.
    pub const NONE: Self = Self {
        brk: LeadingBreak::None,
        bare: LeadingBreak::None,
        multiline: false,
        ungrouped: false,
        first_comment: None,
    };

    /// The ASI reading: the worst line terminator anywhere ahead of the first token —
    /// a line after a comment or ahead of all of them, or a multi-line block's interior.
    #[must_use]
    pub fn asi(self) -> LeadingBreak {
        if self.multiline {
            LeadingBreak::Hard
        } else {
            self.brk.max(self.bare)
        }
    }
}

/// How many nodes [`DocArena::leading_comment_break`] visits, across all of its layouts,
/// before it stops and answers [`LeadingBreak::Hard`] — the safe reading of an edge it
/// could not finish. A leading edge is a handful of nodes; the bound is for a doc whose
/// alternatives multiply ahead of its first token.
const LEADING_EDGE_BUDGET: u32 = 4096;

/// No frame, no mode record: the end of a [`LeadingEdgeWalk`] list.
const EDGE_NIL: u32 = u32::MAX;

/// The group enclosing the walked doc itself, whose mode the doc cannot know — the key a
/// breakable line or an `if_break` outside every group of the doc reads.
const EDGE_OUTSIDE: u32 = u32::MAX;

/// One pending node of a [`LeadingEdgeWalk`] layout: the node, the nearest group it prints
/// in (a group node's id, or [`EDGE_OUTSIDE`]), and the frame printed after it.
#[derive(Clone, Copy)]
struct EdgeFrame {
    node: DocId,
    group: u32,
    next: u32,
}

/// A group's mode as one layout of a [`LeadingEdgeWalk`] has settled it, and the record
/// settled before it.
#[derive(Clone, Copy)]
struct EdgeMode {
    group: u32,
    broken: bool,
    prev: u32,
}

/// One layout of a [`LeadingEdgeWalk`]: what is left to print (a frame list) and what this
/// layout has settled (a mode list). Both lists are shared tails in the walk's stores, so a
/// fork copies two indices.
#[derive(Clone, Copy)]
struct EdgeLayout {
    next: u32,
    modes: u32,
    /// A comment printed ahead of everything so far, a deferred one included.
    comment: bool,
    /// A deferred run is pending: a boundary flushes it with a hard line.
    suffix: bool,
    /// A [`DocNode::FlushBreak`] was passed: the next breakable line breaks.
    flush: bool,
}

/// [`DocArena::leading_comment_break`]'s walk: every layout of the doc's leading edge, in
/// print order, each followed to its first code token.
///
/// Iterative, over persistent lists: frames and mode records are appended and never
/// rewritten, and a layout names the head of each, so forking at an `if_break` or a
/// conditional group's states is two index copies rather than a copy of a stack.
struct LeadingEdgeWalk<'a> {
    arena: &'a DocArena,
    nodes: &'a [DocNode],
    children: &'a [DocId],
    pool: &'a str,
    source: &'a str,
    frames: SmallVec<[EdgeFrame; 32]>,
    modes: SmallVec<[EdgeMode; 8]>,
    layouts: SmallVec<[EdgeLayout; 4]>,
    edge: LeadingEdge,
    budget: u32,
}

impl LeadingEdgeWalk<'_> {
    fn frame(&mut self, node: DocId, group: u32, next: u32) -> u32 {
        crate::fanout_probe::walk();
        let at = self.frames.len() as u32;
        self.frames.push(EdgeFrame { node, group, next });
        at
    }

    fn settle(&mut self, group: u32, broken: bool, prev: u32) -> u32 {
        let at = self.modes.len() as u32;
        self.modes.push(EdgeMode {
            group,
            broken,
            prev,
        });
        at
    }

    /// `group`'s mode in a layout whose mode list starts at `modes`: `Some(broken)` once
    /// the layout has settled it, `None` while either mode is still open.
    fn mode(&self, mut modes: u32, group: u32) -> Option<bool> {
        while modes != EDGE_NIL {
            let record = self.modes[modes as usize];
            if record.group == group {
                return Some(record.broken);
            }
            modes = record.prev;
        }
        None
    }

    /// Walk every layout; `false` when one ended a line in every layout (the answer is
    /// [`LeadingBreak::Hard`] and nothing can raise it), or the budget ran out.
    fn run(&mut self) -> bool {
        while let Some(layout) = self.layouts.pop() {
            if !self.follow(layout) {
                return false;
            }
        }
        true
    }

    /// Follow one layout to its first code token, forking the open alternatives onto
    /// [`Self::layouts`]. `false` to stop the whole walk.
    fn follow(&mut self, mut at: EdgeLayout) -> bool {
        while at.next != EDGE_NIL {
            if self.budget == 0 {
                self.edge.brk = LeadingBreak::Hard;
                return false;
            }
            self.budget -= 1;
            let EdgeFrame { node, group, next } = self.frames[at.next as usize];
            at.next = next;
            match &self.nodes[node.index()] {
                DocNode::Text(t) => {
                    let s = super::types::resolve_text(t, Some(self.source), self.pool)
                        .trim_start_matches([' ', '\t']);
                    if s.is_empty() {
                        continue;
                    }
                    if s.starts_with("//") {
                        // A line comment ends its line in every layout.
                        self.edge.brk = LeadingBreak::Hard;
                        return false;
                    }
                    if !s.starts_with("/*") {
                        return true;
                    }
                    if self.edge.first_comment.is_none()
                        && let DocText::SourceSpan(span, _) | DocText::VerbatimSpan(span, _) = t
                    {
                        self.edge.first_comment = Some(*span);
                    }
                    // Every ECMAScript line terminator, not the `\n` the comment's own
                    // `multiline` flag reads (Svelte's wire gate): a `<LS>` / `<PS>` inside a
                    // block ends a line for ASI just the same.
                    self.edge.multiline |= crate::printing::contains_line_terminator(s);
                    at.comment = true;
                }
                DocNode::MultilineText { span, .. } => {
                    if !span
                        .slice(self.pool)
                        .trim_start_matches([' ', '\t'])
                        .starts_with("/*")
                    {
                        return true;
                    }
                    self.edge.multiline = true;
                    at.comment = true;
                }
                DocNode::Line(kind) => {
                    let broken = match kind {
                        LineKind::Hard | LineKind::Literal => Some(true),
                        LineKind::Normal | LineKind::Soft if at.flush => Some(true),
                        LineKind::Normal | LineKind::Soft => self.mode(at.modes, group),
                    };
                    at.flush = false;
                    match broken {
                        Some(false) => {}
                        Some(true) => {
                            if at.comment {
                                self.edge.brk = LeadingBreak::Hard;
                                return false;
                            }
                            self.edge.bare = LeadingBreak::Hard;
                        }
                        None => {
                            // The layout where the line breaks ends here; the one where it
                            // stays flat goes on, its group settled flat.
                            if at.comment {
                                self.edge.brk = self.edge.brk.max(LeadingBreak::Soft);
                                self.edge.ungrouped |= group == EDGE_OUTSIDE;
                            } else {
                                self.edge.bare = self.edge.bare.max(LeadingBreak::Soft);
                            }
                            at.modes = self.settle(group, false, at.modes);
                        }
                    }
                }
                DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                    at.next = self.frame(*inner, group, at.next);
                }
                DocNode::AlignRoot { contents, .. }
                | DocNode::Align { contents, .. }
                | DocNode::IndentIfBreak { contents, .. }
                | DocNode::GatedState { contents, .. } => {
                    at.next = self.frame(*contents, group, at.next);
                }
                DocNode::WithContext { doc, .. } => {
                    at.next = self.frame(*doc, group, at.next);
                }
                DocNode::Concat(range) | DocNode::Fill(range) => {
                    // A `Fill`'s separators are read as plain lines in its group, not with
                    // its pairwise fit rule — theoretical for a TS leading edge: TS's one fill
                    // (a number array's elements) sits behind its `[`.
                    for &child in range.resolve(self.children).iter().rev() {
                        at.next = self.frame(child, group, at.next);
                    }
                }
                DocNode::Group {
                    contents,
                    expanded_states,
                    should_break,
                    ..
                } => {
                    let id = node.0;
                    // A group printed in a flat one prints flat, unless its own content
                    // forces it open.
                    let parent_flat = self.mode(at.modes, group) == Some(false);
                    let states = expanded_states.resolve(self.children);
                    if states.is_empty() || parent_flat {
                        let forced = *should_break || self.arena.will_break(*contents);
                        if forced || parent_flat {
                            at.modes = self.settle(id, forced, at.modes);
                        }
                        at.next = self.frame(*contents, id, at.next);
                    } else {
                        // A conditional group: each state is a layout of its own.
                        for &state in std::iter::once(contents).chain(states) {
                            let mut fork = at;
                            if *should_break || self.arena.will_break(state) {
                                fork.modes = self.settle(id, true, fork.modes);
                            }
                            fork.next = self.frame(state, id, fork.next);
                            self.layouts.push(fork);
                        }
                        return true;
                    }
                }
                DocNode::IfBreak {
                    break_doc,
                    flat_doc,
                    group_id,
                } => {
                    let key = group_id.map_or(group, |id| id.doc().0);
                    match self.mode(at.modes, key) {
                        Some(true) => at.next = self.frame(*break_doc, group, at.next),
                        Some(false) => at.next = self.frame(*flat_doc, group, at.next),
                        None => {
                            let mut flat = at;
                            flat.modes = self.settle(key, false, flat.modes);
                            flat.next = self.frame(*flat_doc, group, flat.next);
                            self.layouts.push(flat);
                            at.modes = self.settle(key, true, at.modes);
                            at.next = self.frame(*break_doc, group, at.next);
                        }
                    }
                }
                // A deferred run prints at the line's end, behind the first token — but a
                // line that breaks before it leaves the run ahead of that line's end.
                DocNode::LineSuffix(_) => {
                    at.comment = true;
                    at.suffix = true;
                }
                DocNode::LineSuffixBoundary | DocNode::EmbedEnd { .. } => {
                    if at.suffix {
                        self.edge.brk = LeadingBreak::Hard;
                        return false;
                    }
                }
                DocNode::FlushBreak => at.flush = true,
                DocNode::BreakParent | DocNode::FlowProbeEnd => {}
            }
        }
        true
    }
}

/// [`DocArena::gap_comment_line`]'s answer: whether a doc prints a comment on a line of its
/// own in the gap ahead of one token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapCommentLine {
    /// In some layout a comment there has a line break before it and one after it (a `//`
    /// needs only the one before) — a comment a reparse reads as own-line.
    OwnLine,
    /// The token was found, and no layout prints such a comment ahead of it.
    Inline,
    /// The token was not found, or the walk outran its budget.
    Unknown,
}

/// How many `(node, state)` visits [`DocArena::gap_comment_line`] takes before it answers
/// [`GapCommentLine::Unknown`].
const GAP_LINE_BUDGET: u32 = 1 << 16;

/// [`GapLineWalk`]'s state bits, one run of non-code text at a time: a line printed in the
/// run so far; a comment in it printed after such a line, still waiting for one after it;
/// a comment in it with a line on both sides; the same, in the run ahead of the last code
/// token.
const GAP_LINE: u8 = 1;
const GAP_PENDING: u8 = 2;
const GAP_OWN: u8 = 4;
const GAP_OWN_PREV: u8 = 8;

/// A block comment's step in a [`GapLineWalk`] run: it waits for a line after it when one
/// came before it, and one spanning lines leaves the next comment on the line its own
/// interior began, the next pass's reading of a comment on a multi-line block's closing
/// line (`GapTrailScan::fixed`).
const fn gap_block_comment(state: u8, spans_lines: bool) -> u8 {
    let state = if state & GAP_LINE != 0 {
        state | GAP_PENDING
    } else {
        state
    };
    if spans_lines { state | GAP_LINE } else { state }
}

/// [`DocArena::gap_comment_line`]'s walk: every layout in print order, a state per layout,
/// memoized per `(node, state)` into the set of states it can leave.
struct GapLineWalk<'a> {
    nodes: &'a [DocNode],
    children: &'a [DocId],
    pool: &'a str,
    source: &'a str,
    target: u32,
    memo: FxHashMap<(DocId, u8), u16>,
    budget: u32,
    found_inline: bool,
    found_own: bool,
}

impl GapLineWalk<'_> {
    /// The set of states `id` leaves, entered in any state of `states` (a bit per state).
    fn walk(&mut self, id: DocId, states: u16) -> u16 {
        let mut out = 0;
        for state in 0..16u8 {
            if states & (1 << state) != 0 {
                out |= self.walk_one(id, state);
            }
        }
        out
    }

    fn walk_one(&mut self, id: DocId, state: u8) -> u16 {
        crate::fanout_probe::walk();
        if let Some(&known) = self.memo.get(&(id, state)) {
            return known;
        }
        if self.budget == 0 {
            return 1 << state;
        }
        self.budget -= 1;
        let one = |state: u8| 1u16 << state;
        let out = match &self.nodes[id.index()] {
            DocNode::Text(t) => {
                if let DocText::SourceSpan(span, _) | DocText::VerbatimSpan(span, _) = t
                    && span.start == self.target
                {
                    if state & (GAP_OWN | GAP_OWN_PREV) != 0 {
                        self.found_own = true;
                    } else {
                        self.found_inline = true;
                    }
                }
                let s = super::types::resolve_text(t, Some(self.source), self.pool)
                    .trim_start_matches([' ', '\t']);
                if s.is_empty() {
                    one(state)
                } else if s.starts_with("//") {
                    one(if state & GAP_LINE != 0 {
                        state | GAP_OWN
                    } else {
                        state
                    })
                } else if s.starts_with("/*") {
                    one(gap_block_comment(
                        state,
                        crate::printing::contains_line_terminator(s),
                    ))
                } else if state & GAP_OWN != 0 {
                    one(GAP_OWN_PREV)
                } else {
                    one(0)
                }
            }
            DocNode::MultilineText { span, .. } => {
                if span
                    .slice(self.pool)
                    .trim_start_matches([' ', '\t'])
                    .starts_with("/*")
                {
                    one(gap_block_comment(state, true))
                } else if state & GAP_OWN != 0 {
                    one(GAP_OWN_PREV)
                } else {
                    one(0)
                }
            }
            DocNode::Line(_) => one(if state & GAP_PENDING != 0 {
                state | GAP_LINE | GAP_OWN
            } else {
                state | GAP_LINE
            }),
            // A deferred run prints at the end of the line it is on: read in place, where the
            // reparse finds it — on a multi-line block's closing line, say.
            DocNode::LineSuffix(inner) => self.walk_one(*inner, state),
            DocNode::LineSuffixBoundary
            | DocNode::EmbedEnd { .. }
            | DocNode::FlushBreak
            | DocNode::BreakParent
            | DocNode::FlowProbeEnd => one(state),
            DocNode::Indent(inner) | DocNode::Dedent(inner) => self.walk_one(*inner, state),
            DocNode::AlignRoot { contents, .. }
            | DocNode::Align { contents, .. }
            | DocNode::IndentIfBreak { contents, .. }
            | DocNode::GatedState { contents, .. } => self.walk_one(*contents, state),
            DocNode::WithContext { doc, .. } => self.walk_one(*doc, state),
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let mut states = one(state);
                for &kid in range.resolve(self.children) {
                    states = self.walk(kid, states);
                }
                states
            }
            DocNode::Group {
                contents,
                expanded_states,
                ..
            } => {
                let mut out = self.walk_one(*contents, state);
                for &alt in expanded_states.resolve(self.children) {
                    out |= self.walk_one(alt, state);
                }
                out
            }
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                ..
            } => self.walk_one(*break_doc, state) | self.walk_one(*flat_doc, state),
        };
        self.memo.insert((id, state), out);
        out
    }
}

/// Where [`DocArena::glue_leading_comment_lines`]'s walk leaves a doc's leading edge.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Glue {
    /// The first code token printed inside: nothing after it is the edge.
    Code,
    /// No code yet; `comment` when a comment printed ahead of everything so far.
    Open { comment: bool },
}

/// The arena storage [`DocArena::pending_run_at_end`] reads.
struct WalkStore<'a> {
    nodes: &'a [DocNode],
    children: &'a [DocId],
    pool: &'a str,
    source: &'a str,
}

impl WalkStore<'_> {
    /// The state a run is in after `id`, read from `state` — memoized per `(id, state)`,
    /// since an `if_break`'s arms and a conditional group's states each start from one
    /// state and may share a subtree.
    fn suffix_walk(
        &self,
        id: DocId,
        mut state: SuffixWalk,
        memo: &mut WalkMemo<(DocId, SuffixWalk), SuffixWalk>,
    ) -> SuffixWalk {
        crate::fanout_probe::walk();
        let key = (id, state);
        if let Some(known) = memo.visit(key) {
            return known;
        }
        match &self.nodes[id.index()] {
            DocNode::LineSuffix(inner) => {
                if !state.pending() {
                    state.run = RunShape::default();
                    state.width_done = false;
                }
                state.queue = RunQueue::Pending;
                self.suffix_payload(*inner, &mut state);
            }
            DocNode::Line(LineKind::Hard | LineKind::Literal)
            | DocNode::MultilineText { .. }
            | DocNode::LineSuffixBoundary => {
                state.queue = RunQueue::Empty;
                state.flush_armed = false;
            }
            DocNode::Line(LineKind::Soft | LineKind::Normal) => {
                if state.flush_armed {
                    state.queue = RunQueue::Empty;
                    state.flush_armed = false;
                } else if state.pending() {
                    state.queue = RunQueue::MaybeFlushed;
                }
            }
            DocNode::FlushBreak => state.flush_armed = true,
            DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                state = self.suffix_walk(*inner, state, memo);
            }
            DocNode::AlignRoot { contents, .. }
            | DocNode::Align { contents, .. }
            | DocNode::IndentIfBreak { contents, .. }
            | DocNode::GatedState { contents, .. } => {
                state = self.suffix_walk(*contents, state, memo);
            }
            DocNode::Group {
                contents,
                expanded_states,
                ..
            } => {
                let start = state;
                state = self.suffix_walk(*contents, start, memo);
                for &alt in expanded_states.resolve(self.children) {
                    state = SuffixWalk::either(state, self.suffix_walk(alt, start, memo));
                }
            }
            DocNode::WithContext { doc, .. } => {
                state = self.suffix_walk(*doc, state, memo);
            }
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                ..
            } => {
                let broken = self.suffix_walk(*break_doc, state, memo);
                let flat = self.suffix_walk(*flat_doc, state, memo);
                state = SuffixWalk::either(flat, broken);
            }
            DocNode::Concat(range) | DocNode::Fill(range) => {
                for &kid in range.resolve(self.children) {
                    state = self.suffix_walk(kid, state, memo);
                }
            }
            DocNode::Text(_)
            | DocNode::EmbedEnd { .. }
            | DocNode::BreakParent
            | DocNode::FlowProbeEnd => {}
        }
        memo.record(key, state)
    }

    /// What `id` does to the last-leaf reading in front of it ([`LastLeafStep`]) —
    /// memoized per node, since the step is a function of the node alone.
    fn last_leaf(&self, id: DocId, memo: &mut WalkMemo<DocId, LastLeafStep>) -> LastLeafStep {
        crate::fanout_probe::walk();
        if let Some(known) = memo.visit(id) {
            return known;
        }
        let step = match &self.nodes[id.index()] {
            DocNode::Text(t) => {
                let text = super::types::resolve_text(t, Some(self.source), self.pool);
                let text = text.trim_start();
                if text.is_empty() {
                    LastLeafStep::PASS
                } else if text.starts_with("/*") || text.starts_with("//") {
                    LastLeafStep::leaf(LastLeaf::Comment)
                } else {
                    LastLeafStep::leaf(LastLeaf::Code)
                }
            }
            DocNode::MultilineText { .. } => LastLeafStep::leaf(LastLeaf::Comment),
            DocNode::Indent(inner) | DocNode::Dedent(inner) => self.last_leaf(*inner, memo),
            DocNode::AlignRoot { contents, .. }
            | DocNode::Align { contents, .. }
            | DocNode::IndentIfBreak { contents, .. }
            | DocNode::GatedState { contents, .. } => self.last_leaf(*contents, memo),
            DocNode::Group {
                contents,
                expanded_states,
                ..
            } => {
                let mut out = self.last_leaf(*contents, memo);
                for &alt in expanded_states.resolve(self.children) {
                    out = out.either(self.last_leaf(alt, memo));
                }
                out
            }
            DocNode::WithContext { doc, .. } => self.last_leaf(*doc, memo),
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                ..
            } => self
                .last_leaf(*break_doc, memo)
                .either(self.last_leaf(*flat_doc, memo)),
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let mut out = LastLeafStep::PASS;
                for &kid in range.resolve(self.children) {
                    out = out.then(self.last_leaf(kid, memo));
                }
                out
            }
            DocNode::LineSuffix(_)
            | DocNode::Line(_)
            | DocNode::LineSuffixBoundary
            | DocNode::EmbedEnd { .. }
            | DocNode::BreakParent
            | DocNode::FlushBreak
            | DocNode::FlowProbeEnd => LastLeafStep::PASS,
        };
        memo.record(id, step)
    }

    /// Read one suffix payload into the run: its leaves in order, a leading break dropped
    /// (with a space in its place) when the payload glues — nothing, or no `//`, ahead of it.
    fn suffix_payload(&self, id: DocId, state: &mut SuffixWalk) {
        let glues = !state.run.ends_in_line_comment;
        let mut leaves: SmallVec<[DocId; 8]> = SmallVec::new();
        self.leaves(id, &mut leaves);
        let mut opening = true;
        for leaf in leaves {
            match &self.nodes[leaf.index()] {
                DocNode::Line(_) => {
                    if !(opening && glues) {
                        state.width_done = true;
                    }
                }
                DocNode::Text(t) => {
                    let text = super::types::resolve_text(t, Some(self.source), self.pool);
                    if text.is_empty() {
                        continue;
                    }
                    if opening && glues && !state.width_done && !text.starts_with(' ') {
                        // The glued comment's separator, standing in for its dropped break.
                        state.run.widen(1);
                    }
                    opening = false;
                    let is_line = text.starts_with("//");
                    if !state.width_done {
                        let w = match t.cached_width() {
                            CachedWidth::Width(w) => w,
                            CachedWidth::HasNewline { first_width } => first_width,
                        };
                        state.run.widen(w);
                    }
                    if !text.trim().is_empty() {
                        state.run.ends_in_line_comment = is_line;
                    }
                    if is_line {
                        state.run.has_line_comment = true;
                        state.width_done = true;
                    }
                }
                _ => {
                    opening = false;
                    state.run.ends_in_line_comment = false;
                    state.width_done = true;
                }
            }
        }
    }

    /// The emitted leaves of a suffix payload, in order.
    fn leaves(&self, id: DocId, out: &mut SmallVec<[DocId; 8]>) {
        crate::fanout_probe::walk();
        match &self.nodes[id.index()] {
            DocNode::Concat(range) | DocNode::Fill(range) => {
                for &kid in range.resolve(self.children) {
                    self.leaves(kid, out);
                }
            }
            DocNode::Indent(inner) | DocNode::Dedent(inner) => self.leaves(*inner, out),
            DocNode::AlignRoot { contents, .. }
            | DocNode::Align { contents, .. }
            | DocNode::Group { contents, .. } => self.leaves(*contents, out),
            DocNode::BreakParent | DocNode::FlushBreak | DocNode::FlowProbeEnd => {}
            _ => out.push(id),
        }
    }
}

/// Fixed ids of the singleton nodes seeded ahead of [`PRELUDE`]'s statics, in seeding order.
const PRELUDE_EMPTY: u32 = 0;
/// `PRELUDE_LINE + kind as u32` for each [`LineKind`], in discriminant order.
const PRELUDE_LINE: u32 = 1;
const PRELUDE_LINE_SUFFIX_BOUNDARY: u32 = 5;
const PRELUDE_BREAK_PARENT: u32 = 6;
const PRELUDE_FLUSH_BREAK: u32 = 7;
const PRELUDE_FLOW_PROBE_END: u32 = 8;
/// Where the statics start: `PRELUDE[i]` is `DocId(PRELUDE_STATICS + i)`.
const PRELUDE_STATICS: u32 = 9;

/// The id of `s` in the prelude, as a byte match.
///
/// Spelled as a `match` on byte-string patterns rather than a scan of the table:
/// the patterns fold to a constant at a literal call site once `text` inlines
/// (a scan would need the loop fully unrolled to do the same), and for a runtime
/// argument the match compiles to a length switch plus a byte jump table. The
/// arm order is the table's; a test binds the two.
///
/// `inline(always)`: the fold happens only where this body is inlined, and the
/// inline-cost analyzer prices the one-byte arms — a jump table on a loaded
/// byte — at face value, so under plain `#[inline]` a literal `text(".")` kept
/// a call to this function (a three-byte literal folded; a one-byte one did
/// not). Forced, every literal member is a constant; a runtime argument
/// carries the switch and the table at its site.
#[expect(clippy::inline_always)]
#[inline(always)]
fn prelude_index(s: &str) -> Option<u32> {
    let i = match s.as_bytes() {
        b"(" => 0,
        b")" => 1,
        b" " => 2,
        b";" => 3,
        b"," => 4,
        b"." => 5,
        b"}" => 6,
        b"{" => 7,
        b":" => 8,
        b"\t" => 9,
        b"]" => 10,
        b"[" => 11,
        b"<" => 12,
        b">" => 13,
        b"=" => 14,
        b"!" => 15,
        b"?" => 16,
        b"`" => 17,
        b"&" => 18,
        b"*" => 19,
        b"+" => 20,
        b"-" => 21,
        b"/" => 22,
        b"|" => 23,
        b"~" => 24,
        b"\"" => 25,
        b": " => 26,
        b", " => 27,
        b" =" => 28,
        b" =>" => 29,
        b" = " => 30,
        b"</" => 31,
        b"=\"" => 32,
        b"/>" => 33,
        b"{ " => 34,
        b" }" => 35,
        b"| " => 36,
        b" (" => 37,
        b") " => 38,
        b"${" => 39,
        b"?." => 40,
        b"..." => 41,
        b"{}" => 42,
        b"> " => 43,
        b" &" => 44,
        b"=>" => 45,
        _ => return None,
    };
    Some(PRELUDE_STATICS + i)
}

/// Seed the singletons and [`PRELUDE`] into a freshly cleared node store, so their ids
/// are fixed.
fn seed_prelude(nodes: &mut Vec<DocNode>) {
    debug_assert!(nodes.is_empty());
    nodes.extend([
        DocNode::Text(DocText::Static("", 0)),
        DocNode::Line(LineKind::Normal),
        DocNode::Line(LineKind::Soft),
        DocNode::Line(LineKind::Hard),
        DocNode::Line(LineKind::Literal),
        DocNode::LineSuffixBoundary,
        DocNode::BreakParent,
        DocNode::FlushBreak,
        DocNode::FlowProbeEnd,
    ]);
    debug_assert_eq!(nodes.len(), PRELUDE_STATICS as usize);
    nodes.extend(
        PRELUDE
            .iter()
            .map(|&(s, width)| DocNode::Text(DocText::Static(s, width))),
    );
}

/// Arena allocator for document nodes.
///
/// All doc nodes are stored contiguously in `nodes`. Multi-child nodes
/// (Concat, Fill, expanded_states) store their children in `children`
/// and reference them via `ChildRange`.
///
/// Uses `RefCell` for interior mutability - builder methods take `&self`
/// to match the existing printer pattern where methods are `&self`.
pub struct DocArena {
    nodes: RefCell<Vec<DocNode>>,
    children: RefCell<Vec<DocId>>,
    /// Backing store for dynamically-built text ([`DocText::Pooled`] and
    /// [`DocNode::MultilineText`] bodies), referenced by [`PoolSpan`]. Keeping
    /// the bytes here instead of per-node `String`s leaves `DocNode` with no
    /// drop glue, so `reset()`/drop clear the node store without walking it.
    /// Grows organically (pooled text is rare — no pre-size) and is rewound by
    /// `reset()` like every other store.
    text_pool: RefCell<String>,
    /// Parked scratch buffer backing [`Self::pool_writer`]: taken (moved out)
    /// by each writer and returned on finish with its capacity retained, so
    /// streamed pooled-text assembly is allocation-free once warm — the same
    /// amortization as the pool itself. Logically empty whenever parked; a
    /// nested writer takes the `Cell`'s empty default and simply warms its own
    /// buffer. Survives `reset()` (always empty between uses; only capacity
    /// persists).
    pool_scratch: Cell<String>,
    /// Parked per-render output scratch backing the printers' render-and-write
    /// seams (`write_arena_doc` / `render_doc_immediate`): taken (moved out)
    /// per render via [`Self::take_render_scratch`], rendered into, copied into
    /// the printer's output buffer, and returned via
    /// [`Self::park_render_scratch`] with capacity retained — so the
    /// per-statement output `String` is allocation-free once warm, the render
    /// analog of `pool_scratch`. Logically empty whenever parked; a nested
    /// render takes the `Cell`'s empty default and simply warms its own buffer
    /// (fresh-fallback, so re-entrancy costs an alloc but stays correct).
    /// Survives `reset()` (always empty between uses; only capacity persists).
    render_scratch: Cell<String>,
    /// The top-level render's pending-command stack, borrowed for the duration of
    /// one `render_doc_iterative` so its capacity warms once per arena instead of
    /// re-allocating per rendered piece. A `RefCell` rather than a `Cell` on
    /// purpose: only the top-level render borrows it, so the held `RefMut` makes
    /// that exclusivity a checked claim — a violated nesting assumption panics
    /// loudly instead of silently handing out an empty stack. Cleared at each
    /// borrow; capacity survives `reset()`.
    top_render_stack: RefCell<CmdStack>,
    /// The nested renders' command stack — see [`Self::take_sub_render_stack`].
    ///
    /// ⚠️ **Deliberately a SECOND slot, and merging it with `top_render_stack`
    /// would undo most of the lever that created it.** A sub-render runs inside
    /// the top-level one, so with a single slot the top-level render holds it for
    /// the whole render and every sub-render grows a fresh `Vec` from nothing —
    /// which is exactly the unparked shape, measured at −0.078% against this
    /// one's −0.917% on fuz_ui. Two slots keep both warm.
    sub_render_stack: Cell<CmdStack>,
    /// The top-level render's deferred line-suffix buffer — the
    /// [`LineSuffixBuf`] companion of `top_render_stack`, on the same borrow
    /// discipline. Sub-renders take a caller-provided buffer instead.
    line_suffix_scratch: RefCell<LineSuffixBuf>,
    /// Parked line-offset scratch for the multi-line block-comment builders:
    /// one `split('\n')` pass per comment fills it with each body line's
    /// `(start, end)` byte range, so the classifier and builder iterate the
    /// lines slice-cheap without materializing a per-comment line buffer.
    /// Cleared at each borrow; capacity survives `reset()`.
    line_spans_scratch: RefCell<Vec<(u32, u32)>>,
    /// Parked whole-source line-break table behind each `format_ast_in`'s
    /// `printing::LineBreaks` — taken (moved out), filled only if a line question
    /// falls back to the table, and parked back cleared with capacity retained,
    /// like `render_scratch`.
    line_breaks_scratch: Cell<Vec<u32>>,
    /// Free-list of reusable [`DocBuf`] assembly buffers for the wide-list doc
    /// builders (statement lists, object/array/call-arg lists, member chains).
    /// A builder assembling a variable-length parts list `acquire`s a cleared
    /// buffer (with retained heap capacity from a prior spill) and `release`s it
    /// on scope exit via the [`PooledDocBuf`] guard. A recursion-safe
    /// pop-or-new pool, holding **only spilled buffers** — a release drops a
    /// never-spilled buffer instead of pooling it (nothing to retain, free to
    /// re-construct), so every pooled entry carries real heap capacity and the
    /// LIFO can't hand a virgin buffer to a big-need builder while a spilled
    /// one sits deeper. Self-sizes to the max concurrent-live spilled buffers,
    /// turning the per-spill `SmallVec` malloc/free churn into a handful of
    /// long-lived reused allocations. Retained across `reset()` (reused across
    /// files), like the other scratches; only ever affects allocation, never
    /// output.
    docbuf_pool: RefCell<Vec<DocBuf>>,
    /// The parked member-chain argument share ([`ChainShareStore`]: an AST-node
    /// pointer plus a consumer-owned build tag (`(usize, u8)`) → the build already
    /// recorded for it: its `DocId` and the side effect a hit replays). The tag is what lets
    /// one node hold **several** entries — the same argument built under different printer
    /// state, or by a different builder
    /// — so the consumer's "a hit is byte-identical to a rebuild" rule is carried by
    /// the key rather than by refusing to cache. Parked here — on the reused
    /// per-thread arena — so both tiers' capacity warms once instead of a fresh
    /// table per printer/file. The consumer owns the protocol: it clears the store
    /// at every share-scope entry AND exit, so between scopes it is logically empty
    /// (stale `DocId`s from a prior document are unreachable — cleared before any
    /// read) and only capacity persists across `reset()`. Only ever affects
    /// allocation, never output (a hit is byte-identical to a rebuild by
    /// construction of the consumer's key); the hashed tier's `FxHasher` is
    /// unobservable for the same reason (see `hash`'s module docs).
    chain_share_store: RefCell<ChainShareStore>,
    /// Memoized per-subtree layout facts, indexed by `DocId` — the forced-break
    /// verdict [`Self::will_break`] answers at BUILD and the flat width
    /// `arena_fits` answers at RENDER, packed into one `u32` per node (the cell
    /// encoding is on [`LAYOUT_UNKNOWN`]). Lazily extended to match `nodes`;
    /// sound because nodes are append-only and the arena is per-format, so a
    /// node's layout facts never change once it exists.
    ///
    /// ⭐ **One cache because one walk.** The two questions used to have a memo
    /// and a recursive fill each, and the two fills traversed the same tree
    /// twice — 144.9% of node-visits against a 75.9% union, with 98.9% of the
    /// flat-width fills landing on a node the build-time break walk had already
    /// visited. [`Self::subtree_layout_fill`] answers both in one pass, so the
    /// second walk is a cache read; the caches merge because they now have
    /// identical populations by construction.
    layout_cache: RefCell<Vec<u32>>,
    /// Direct-mapped cache for [`Self::text`] statics, carrying two halves per
    /// slot with different lifetimes:
    ///
    /// - **Width half** (`ptr`/`len` → `width`): a static string's precomputed
    ///   visual width, so `Static` nodes carry a real cached width (fits
    ///   answers from the node, `render_text` skips its column byte-scan)
    ///   while the width is *measured* only once per unique string per arena —
    ///   never per node (the per-node eager measure was a measured loss).
    ///   Entries are `'static`-valid, so this half survives `reset()` and
    ///   warms once per arena lifetime.
    /// - **Node half** (`node_gen` → `node_id`): the interned `Static` node
    ///   for the *current document*, valid only while `node_gen` matches
    ///   [`Self::format_gen`] — repeated `text(",")` calls within one format
    ///   return one shared node instead of allocating per call (statics are
    ///   position-free at render and nodes are append-only/immutable, so
    ///   sharing is output-identical; `join_doc` shares separator ids the same
    ///   way). `reset()` invalidates every node half in O(1) by bumping the
    ///   generation; the width half deliberately survives.
    ///
    /// A collision evict just re-measures and re-allocs, and the eviction
    /// *rate* is small — but the rate is the wrong statistic and the slot count
    /// is sized against the *draw* instead: see [`STATIC_CACHE_SLOTS`], which
    /// records what a bad draw costs and why the index is re-rolled on every
    /// exec. `Cell` (no borrow flag) —
    /// probes never alias the `RefCell` stores. **Boxed**, not inline: inline,
    /// its 48 KB sat in the frame of whatever held the arena — a one-shot
    /// `format` paid it as a fixed stack floor beneath the whole recursive
    /// descent, and the constructor's by-value return paid it again in its own
    /// frame. Behind the box the probe pays one dependent load (the pointer,
    /// beside the rest of `self`).
    static_cache: Box<[StaticSlot; STATIC_CACHE_SLOTS]>,
    /// The current document's format generation, keying the validity of the
    /// interned node halves in `static_cache`. Starts
    /// at 1 (0 marks a never-stamped slot) and is bumped by `reset()`, so a
    /// prior document's `node_id`s — invalidated by the reset — can never be
    /// returned for the new document.
    format_gen: Cell<u32>,
    /// Flow-probe render state: the output-length snapshots of the probes currently open
    /// (a stack — probed subtrees nest), plus the most recently completed probe's answer.
    /// Written only by the render loop (probe begin at a flagged node, finish at its
    /// [`DocNode::FlowProbeEnd`] sentinel), read by
    /// [`DocContext::hold_line_after_broken_flow`]'s fill hook. Positional freshness: the
    /// sentinel completes immediately before the paired fill's command, so the answer the
    /// fill reads is always its own predecessor's. Measurement walks never touch it —
    /// `arena_fits` skips the sentinel — so a nested measure render cannot corrupt an open
    /// probe (a measure render that *contains* a probed node balances its own begin/finish
    /// before the enclosing render continues).
    flow_probe: RefCell<FlowProbeState>,
    /// The most recently completed flow probe's answer — split out of `flow_probe` so the
    /// per-fill read is a `Cell` load, not a `RefCell` borrow.
    flow_probe_broke: Cell<bool>,
    /// Debug-only tripwire on the flow probe's POSITIONAL-PAIRING invariant: set by
    /// [`Self::flow_probe_finish`], cleared by [`Self::flow_probe_consume`]. A hold-flagged
    /// fill must render immediately after its paired probe's sentinel; a consume finding no
    /// fresh answer means a builder flagged a fill without pairing it (or its probe was
    /// skipped), and the hook would otherwise act silently on a stale answer. One-directional
    /// by design — a probe whose paired fill never consumes leaves a stale-true flag the next
    /// legitimate pair overwrites, so only the hold-without-probe miswire is caught.
    #[cfg(debug_assertions)]
    flow_probe_fresh: Cell<bool>,
    /// The mode each keyed group resolved to, per group INSTANCE ([`GroupId`], the group's
    /// own node) — prettier's `groupModeMap`, one map for the whole render. Recorded in
    /// resolution order as `(group, broke)`, keyed on the group's bare [`DocId`]; a reader
    /// searches from the newest entry. An id with no entry (a group not yet resolved in
    /// this render) reads flat — prettier's `fits` default (`groupModeMap[id] ||
    /// MODE_FLAT`); prettier's printer instead prints NEITHER arm of an `ifBreak` for an
    /// unresolved id and drops an `indentIfBreak`'s contents. No real input reaches a miss:
    /// every reader renders after the group it keys on.
    ///
    /// Per instance, not per kind: a reader keyed on one group must not take the mode of
    /// another group of the same shape resolved between them — a curried chain nested in
    /// an outer chain's parameter default or body resolves after the outer chain's heads
    /// and before the outer's body indent / callee `)` reads them. The newest-first search
    /// finds its group at or next to the newest entry on real code, because a reader sits
    /// close behind its group (the fluid assignment marker's `indent_if_break` is the next
    /// keyed read after it — only a `line_suffix_boundary` sits between), and a group
    /// rendered twice in one render reads its latest resolution, as prettier's map does.
    ///
    /// It lives on the arena, not on a render's policy, because a render is not one loop:
    /// every fill item and every line-suffix flush renders through a nested sub-render,
    /// and a keyed `if_break` / `indent_if_break` inside a fill item must read the group
    /// its item resolved (a Svelte block head's `}` dangle, a TS curried chain's body
    /// indent). A map scoped to one loop would read every keyed group in a sub-render as flat.
    ///
    /// Owned by the top-level render, which clears it on entry
    /// ([`Self::clear_keyed_groups`]). Written only by the render loop when it resolves a
    /// keyed group; `arena_fits` never reads or writes it (a fits walk measures a keyed
    /// conditional as flat).
    keyed_group_modes: RefCell<Vec<(DocId, bool)>>,
    /// Diagnostic side-set: indices of text nodes that are line comments,
    /// recorded by `line_comment_text_pooled` only while the swallow check is
    /// enabled (empty and untouched otherwise). Appended in `alloc` order, so
    /// the vec is sorted ascending — the renderer membership-tests via binary
    /// search. See [`super::swallow`]. Compiled in only under the `swallow_check`
    /// feature, so production builds carry no diagnostic state.
    #[cfg(feature = "swallow_check")]
    line_comment_ids: RefCell<Vec<u32>>,
    /// Diagnostic side-set: the doc nodes that *are* a comment, recorded by
    /// [`Self::tag_comment_doc`] only while the comment ledger is enabled (empty and
    /// untouched otherwise). Each entry pairs the node with the comment's span and the
    /// document it was parsed from, because the renderer — which records the emit when it
    /// reaches the node — holds no `source`. Appended in `alloc` order, so the vec is
    /// sorted ascending on the id and the renderer looks up by binary search. See
    /// [`crate::comment_ledger`]. Compiled in only under the `comment_check` feature.
    #[cfg(feature = "comment_check")]
    comment_docs: RefCell<Vec<(u32, Span, DocumentKey)>>,
}

/// Flow-probe render state — see the `flow_probe` field's doc on [`DocArena`].
/// The answer bit lives beside it as a plain `Cell` ([`DocArena::flow_probe_broke`]'s read
/// takes no `RefCell` borrow).
#[derive(Default)]
struct FlowProbeState {
    /// Output-length snapshots of the probes currently open, innermost last.
    starts: SmallVec<[usize; 4]>,
}

/// One `static_cache` slot: a static string's identity (`ptr`+`len`)
/// mapped to its precomputed width, plus the per-document interned node
/// (`node_id`, valid iff `node_gen` matches the arena's `format_gen`). The
/// `len` compare is load-bearing, not belt-and-braces: linker
/// constant-merging can make one static share another's *start* pointer
/// (prefix overlap), so `ptr` alone is not identity — `ptr`+`len` is (same
/// address + same length ⇒ same bytes). That same identity argument covers
/// the node half: identical `ptr`+`len` ⇒ the same `&'static str`, so the
/// interned node's stored text is indistinguishable from the caller's.
///
/// A struct of `Cell`s, not a `Cell` of a struct: the hit path in [`DocArena::text`]
/// reads the three fields it compares and the one it returns straight from the
/// slot. As one `Cell<StaticSlot>`, `get()` copied all 24 bytes to a stack
/// temporary that the compares then re-read — two stores and a store-forward
/// round trip on every probe, at every inlined site — because the by-value
/// `slot` argument to the cold miss kept the copy live on the hot path. The
/// miss re-reads the slot it is handed the index of.
#[derive(Debug)]
struct StaticSlot {
    ptr: Cell<usize>,
    len: Cell<u32>,
    width: Cell<u16>,
    /// Format generation that stamped `node_id`; 0 = never stamped.
    node_gen: Cell<u32>,
    /// The interned node for the generation in `node_gen`.
    node_id: Cell<DocId>,
}

impl StaticSlot {
    /// An empty slot: `ptr == 0` is never a real entry (references are never
    /// null — even `""` has a non-null dangling address).
    const fn empty() -> Self {
        Self {
            ptr: Cell::new(0),
            len: Cell::new(0),
            width: Cell::new(0),
            node_gen: Cell::new(0),
            node_id: Cell::new(DocId(0)),
        }
    }
}

/// 2048 slots (× 24 B on 64-bit = 48 KB, boxed). Kept in lockstep with the
/// slot-hash shift — see the assert below.
///
/// **Sized against the collision draw, not against the population.** The
/// unique-static population is ~165–190 across real corpora and the *per
/// document* working set is ~55 (before [`PRELUDE`] took the hottest 46 — and
/// three quarters of the calls — out of the cache entirely; what reaches it now
/// is the long tail, keywords mostly), so a 512-slot table looks like ten times the
/// room it needs — and the eviction rate it produces is genuinely small
/// (≤0.7% of `text()` calls). That framing is what made 512 look sufficient,
/// and it measures the wrong thing: the cost is not the *rate*, it is **which
/// pair collides**. Two hot statics landing in one slot thrash — every call to
/// either one misses, re-measures the width and allocates a fresh node — and a
/// single such pair moves the whole run.
///
/// The index is a hash of the string's **runtime address**, so the draw is
/// re-rolled by any change to the link layout (a one-line edit anywhere, a
/// linker flag) and, under PIE + ASLR, **again on every exec**: whether two
/// statics collide is decided by their offset *difference* — a link-time
/// constant — but the pairs sitting near the hash's wraparound flip with the
/// image base. Measured at 512 slots, one binary (`tsv_debug`) ran a 0.53%
/// spread in `instructions:u` across 14 execs of the *same binary on the same
/// input*, bimodal, and its `DocArena` node population moved with it
/// (`arena_stats` over 341 files: 18,669 `Static` nodes at the floor, up to
/// 30,994 on a bad draw). At 2048 the same binary spreads 0.013% and the
/// population is within 1.2% of the floor. The medians move
/// **−0.36…−0.63%** (`profile` over fuz_app / zzz / gro / fuz_ui) against
/// parse-only boards — which build no docs — at +0.000%.
///
/// So the array is sized to make the draw stop mattering: 1024 already
/// recovers the medians (−0.50…−0.62%) but keeps a 0.15% tail; 2048 removes
/// the tail; 4096 is worth a further −0.01% and is not worth 96 KB. The 48 KB
/// is one heap allocation per arena (the `static_cache` field's box), which a
/// multi-file driver and a binding's per-thread arena make once.
const STATIC_CACHE_SLOTS: usize = 2048;

// The slot index is the TOP 11 BITS of the 64-bit multiplicative hash
// (`>> 53` in `text`), which is provably `< 2048` — that both elides
// the array bounds check and hard-couples the shift to the slot count. This
// assert makes changing one without the other a compile error.
const _: () = assert!(STATIC_CACHE_SLOTS == 1 << 11);

/// A fresh [`DocArena::static_cache`]: every slot empty, written straight into its heap
/// allocation. `Box::new` of the array literal would build it on the stack first (48 KB of
/// frame, then a copy), the floor the box exists to remove.
fn empty_static_cache() -> Box<[StaticSlot; STATIC_CACHE_SLOTS]> {
    let mut slots = Vec::with_capacity(STATIC_CACHE_SLOTS);
    slots.resize_with(STATIC_CACHE_SLOTS, StaticSlot::empty);
    // the vector holds exactly `STATIC_CACHE_SLOTS` slots, so the conversion cannot fail
    #[expect(clippy::unreachable)]
    let cache = slots
        .into_boxed_slice()
        .try_into()
        .unwrap_or_else(|_| unreachable!());
    cache
}

/// How one render-stack entry classifies for the flow-boundary look-ahead's welded-unit walk
/// ([`DocArena::welded_entry`]; consumed by `flow_lookahead` in `arena_render_fill`).
#[derive(Debug, Clone, Copy)]
pub(crate) enum WeldedEntry {
    /// Not byte-glued to what precedes it on the render stack — the welded unit ended before this
    /// entry, and the walk stops.
    NotGlued,
    /// A glued text **run**: its head alone is pinned, its internal whitespace boundaries are
    /// ordinary break points. It rides in its own inherited mode and the walk continues past it.
    TextRun,
    /// A glued breakable **atom** (the payload is the doc to measure — an after-element fold's
    /// lead, or a bare glued element / glued element run): measured **flat**, and it ENDS the
    /// unit, since whatever follows it sits behind a break opportunity of its own.
    Atom(DocId),
}

impl DocArena {
    /// Create a new empty arena.
    pub fn new() -> Self {
        let mut arena = Self {
            nodes: RefCell::new(Vec::new()),
            children: RefCell::new(Vec::new()),
            text_pool: RefCell::new(String::new()),
            pool_scratch: Cell::new(String::new()),
            render_scratch: Cell::new(String::new()),
            top_render_stack: RefCell::new(Vec::new()),
            sub_render_stack: Cell::new(Vec::new()),
            line_suffix_scratch: RefCell::new(SmallVec::new()),
            line_spans_scratch: RefCell::new(Vec::new()),
            line_breaks_scratch: Cell::new(Vec::new()),
            docbuf_pool: RefCell::new(Vec::new()),
            chain_share_store: RefCell::new(ChainShareStore::default()),
            layout_cache: RefCell::new(Vec::new()),
            static_cache: empty_static_cache(),
            format_gen: Cell::new(1),
            flow_probe: RefCell::new(FlowProbeState::default()),
            flow_probe_broke: Cell::new(false),
            #[cfg(debug_assertions)]
            flow_probe_fresh: Cell::new(false),
            keyed_group_modes: RefCell::new(Vec::new()),
            #[cfg(feature = "swallow_check")]
            line_comment_ids: RefCell::new(Vec::new()),
            #[cfg(feature = "comment_check")]
            comment_docs: RefCell::new(Vec::new()),
        };
        seed_prelude(arena.nodes.get_mut());
        arena
    }

    /// Create an arena with pre-allocated capacity based on source size.
    ///
    /// Heuristic: **~2 doc nodes per source byte**. Node interning (static
    /// text, then the Line/boundary singletons) cut real node density to
    /// roughly half the pre-interning level (measured on the anchor corpora:
    /// ~0.25–0.26 nodes/byte mean, p99 ~0.6–1.0, max ~1.2), so 2/byte clears
    /// the densest file outright. It is deliberately NOT
    /// lowered to match: `estimated_children = nodes/2` ⇒ ~1/byte, and the
    /// children population is untouched by interning (shared nodes still
    /// appear once per use in child lists — children/byte p99 ~0.90), so
    /// halving the node hint would drag the children hint below real demand.
    /// The pre-size only ever sets `Vec` capacity — never output — so tuning it
    /// is byte-identical; the win is the fresh-arena / first-file / WASM
    /// reservation, and the multi-file `reset()` reuse high-water is bounded by
    /// actual usage, so it can only drop.
    pub fn with_source_size_hint(source_len: usize) -> Self {
        let estimated_nodes = source_len * 2;
        let estimated_children = estimated_nodes / 2;
        let mut arena = Self {
            nodes: RefCell::new(Vec::with_capacity(estimated_nodes)),
            children: RefCell::new(Vec::with_capacity(estimated_children)),
            // Pooled text is rare (~1.4% of Text nodes) but its bytes are not
            // negligible: measured per-file pool demand is p50 ≈ 0.17× source /
            // p90 ≈ 0.57× (MultilineText comment bodies dominate). A `len/8`
            // floor absorbs the growth chain's first ~7 doublings on fresh
            // arenas without inflating the reuse high-water (reset() retains
            // organic capacity, bounded by the largest file's demand).
            text_pool: RefCell::new(String::with_capacity(source_len / 8)),
            pool_scratch: Cell::new(String::new()),
            render_scratch: Cell::new(String::new()),
            top_render_stack: RefCell::new(Vec::new()),
            sub_render_stack: Cell::new(Vec::new()),
            line_suffix_scratch: RefCell::new(SmallVec::new()),
            line_spans_scratch: RefCell::new(Vec::new()),
            line_breaks_scratch: Cell::new(Vec::new()),
            docbuf_pool: RefCell::new(Vec::new()),
            chain_share_store: RefCell::new(ChainShareStore::default()),
            // The fitting memos top out at `nodes.len()` (~= `estimated_nodes`),
            // growing from 0 via repeated `resize(nodes.len(), …)`; pre-reserve
            // to absorb those reallocs. Only capacity changes — never values.
            layout_cache: RefCell::new(Vec::with_capacity(estimated_nodes)),
            static_cache: empty_static_cache(),
            format_gen: Cell::new(1),
            flow_probe: RefCell::new(FlowProbeState::default()),
            flow_probe_broke: Cell::new(false),
            #[cfg(debug_assertions)]
            flow_probe_fresh: Cell::new(false),
            keyed_group_modes: RefCell::new(Vec::new()),
            #[cfg(feature = "swallow_check")]
            line_comment_ids: RefCell::new(Vec::new()),
            #[cfg(feature = "comment_check")]
            comment_docs: RefCell::new(Vec::new()),
        };
        seed_prelude(arena.nodes.get_mut());
        arena
    }

    /// Create an arena sized for `source`.
    ///
    /// Equivalent to `with_source_size_hint(source.len())`.
    pub fn for_source(source: &str) -> Self {
        Self::with_source_size_hint(source.len())
    }

    /// Reset the arena for reuse on the next document, retaining capacity.
    ///
    /// Clears every backing store (nodes, children, and the fitting memos) but
    /// keeps each `Vec`'s allocated capacity, so a driver that formats many
    /// files allocates the buffers once and rewinds between files — the doc-IR
    /// analogue of the per-call AST `Bump::reset()` reuse in the FFI/CLI
    /// bindings. Only the first file (and any that grow past the high-water
    /// mark) pays a (re)allocation; the rest reuse the retained buffers.
    ///
    /// Sound to call only between documents: every `DocId` handed out for the
    /// previous document is invalidated (ids restart at 0), so no `DocId` from a
    /// prior render may be read after a reset. `&mut self` enforces this — no
    /// borrow of the arena's contents can be live across the call.
    ///
    /// The static cache's *width* halves are deliberately NOT cleared:
    /// they key on `'static` string addresses, so they stay valid for the
    /// arena's whole lifetime and the cache warms once across documents. The
    /// interned *node* halves are invalidated in O(1) by bumping `format_gen` —
    /// their `DocId`s point into the node store this method just cleared — and
    /// the prelude is re-seeded at its fixed ids.
    pub fn reset(&mut self) {
        let next = self.format_gen.get().wrapping_add(1);
        if next == 0 {
            // u32 generation wrap (~4.3 B resets in one process): a slot last
            // stamped in the ancient generation with this same value would
            // false-hit and return a dangling id, so hard-clear every node
            // half once per wrap. The width halves stay valid ('static-keyed).
            for slot in self.static_cache.iter() {
                slot.node_gen.set(0);
            }
            self.format_gen.set(1);
        } else {
            self.format_gen.set(next);
        }
        self.nodes.get_mut().clear();
        seed_prelude(self.nodes.get_mut());
        self.children.get_mut().clear();
        self.text_pool.get_mut().clear();
        self.layout_cache.get_mut().clear();
        #[cfg(feature = "swallow_check")]
        self.line_comment_ids.get_mut().clear();
        #[cfg(feature = "comment_check")]
        self.comment_docs.get_mut().clear();
    }

    //
    // Internal helpers
    //

    /// Allocate a node and return its DocId.
    #[inline]
    fn alloc(&self, node: DocNode) -> DocId {
        // the one allocation site, so the fanout budget bounds every printer's doc
        crate::fanout_probe::node();
        let mut nodes = self.nodes.borrow_mut();
        let id = DocId(nodes.len() as u32);
        nodes.push(node);
        id
    }

    /// Append `s` to the arena text pool and return its span.
    #[inline]
    fn pool_push(&self, s: &str) -> PoolSpan {
        let mut pool = self.text_pool.borrow_mut();
        let start = pool.len() as u32;
        pool.push_str(s);
        PoolSpan {
            start,
            len: s.len() as u32,
        }
    }

    /// Allocate a child range from a slice of DocIds.
    ///
    /// ⚠️ **The append is deliberately NOT `specialize_short_len!`-specialized
    /// here, and the reason is a caller, not a length distribution.** This site
    /// carried a `[2]` arm and then a `[2, 3]` one, both justified by a census
    /// saying a child range holds exactly two ids in 65% of calls. That census
    /// is still true and no longer bears on this `match`: [`Self::concat`]
    /// routes every two-child range to [`Self::concat_pair`], which calls this
    /// function with the **literal** `&[a, b]`, so at the site that owns the 65%
    /// the length is a compile-time constant and the arm folds away whether or
    /// not it is written. What is left matching at run time is
    /// [`Self::concat_other`] — reached only with **zero or three-plus**
    /// children, so a `2` arm there can never fire — plus [`Self::fill`],
    /// `conditional_group`'s expanded states and the line-removal rebuild.
    /// Splitting `concat` moved the population out from under the ladder, and
    /// the arms kept being added to it afterwards.
    ///
    /// Measured by removing them, four builds a side sampling **code layout**
    /// (see `specialize_short_len!`'s ladder note for why that and not a hash
    /// constant): dropping `3` is `instructions:u` **−0.25…−0.31%** on four real
    /// corpora, dropping both is **−0.43…−0.52%**, each against a parse+bind
    /// null control at −0.001% and a within-group spread of 0.000–0.003%. On
    /// `cycles:u` the ladder never paid: every rung reads neutral-to-favourable
    /// on removal (−0.01…−0.53%, four corpora, against a same-source null of
    /// +0.17%), and wall agrees at −0.29%. So the arms were not buying the
    /// memory behaviour they were credited with. `.text` **−5,648 B**; the
    /// `format` WASM bundle moves the other way by **+143 B**, and `parse` is
    /// byte-identical, as it must be — it links no doc builder.
    #[inline]
    fn alloc_children(&self, ids: &[DocId]) -> ChildRange {
        if ids.is_empty() {
            return ChildRange::EMPTY;
        }
        let mut children = self.children.borrow_mut();
        let start = children.len() as u32;
        let len = ids.len() as u32;
        children.extend_from_slice(ids);
        ChildRange { start, len }
    }

    //
    // Primitive builders
    //

    /// Create a text doc from a static string (zero allocation), interned
    /// per document — or, for a [`PRELUDE`] member, a fixed id.
    ///
    /// Repeated calls with the same static within one format return one
    /// shared node (`text(",")` ×10 K → 1 node). The hottest statics — the
    /// punctuation that is three quarters of a format run's `text()` calls —
    /// are the prelude, seeded at fixed ids ahead of every document, and a
    /// literal call site folds to that id with no probe at all. The rest go
    /// through the direct-mapped slot, which carries the interned `DocId`
    /// alongside the cached width, gated by the arena's `format_gen` so a
    /// `reset()` invalidates every interned node in O(1). Sharing is
    /// output-identical — statics are position-free at render, nodes are
    /// append-only and immutable, and no consumer keys on an interned node's
    /// identity (the identity-keyed tables — the keyed-group map, the swallow and
    /// comment side-sets — key only on nodes that are never interned;
    /// `join_doc` has always shared separator ids). The width half is
    /// amortized the same way as before (measured once per unique string per
    /// arena *lifetime* — the *per-node* eager measure was a measured loss);
    /// fits queries answer from the node alone and `render_text`'s column
    /// advance skips its byte scan.
    ///
    /// Hot path of the interned half: the slot hash, three field loads, and a
    /// ptr/len/gen compare — field loads,
    /// not a slot copy: as one `Cell<StaticSlot>` the 24-byte `get()` went
    /// through a stack temporary at every inlined site. ⚠️ **The hash does not fold.** The
    /// tempting reading is that `s`'s address is a link-time constant, so a
    /// folded call site should carry a literal index — but every shipped target
    /// is PIE, where the address is `image_base + link_offset` and the base is
    /// not known until `execve`. `objdump` shows the arithmetic emitted whole at
    /// every site the interned half inlines into (`lea` the RIP-relative address,
    /// `movabs` the constant, `imul`, `shr`), and the consequence is bigger than
    /// four instructions: the slot index is **re-drawn on every exec**, which is
    /// what [`STATIC_CACHE_SLOTS`] is sized against. The miss path — first use
    /// this document, first sighting ever, or collision evict — allocs and
    /// restamps in the cold helper.
    ///
    /// `inline(always)`: the prelude match is what this wrapper is for, and it
    /// folds only where the wrapper is inlined — the inline-cost analyzer prices
    /// the match and the probe behind it at face value (it does not fold a load
    /// from a constant global), and under plain `#[inline]` LLVM declined the
    /// body at the literal sites, leaving each one a call plus the runtime
    /// match: `.text` fell, instructions did not. Forced, a literal member is a
    /// constant and a literal non-member a direct call to [`Self::text_interned`].
    ///
    /// ⚠️ **The fold needs a literal AT THE CALL SITE.** A `&'static str` that reaches
    /// this call as a runtime value — a builder's parameter, an enum's `as_str()`, a
    /// trimmed const — is the length switch and the byte jump table at that site, and
    /// when its value alternates per caller the jump mispredicts: ~48 cycles a call
    /// where the instruction count says ~8. A builder that receives a delimiter or an
    /// operator therefore takes it as a `DocId` its caller built where the literal is
    /// spelled (`d.text(":")`, `d.text("?.(")`), and emits the node it was handed; a
    /// keyword table's values are longer than any prelude member and fall out of the
    /// switch on one predictable compare, so those sites keep the string.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    pub fn text(&self, s: &'static str) -> DocId {
        match prelude_index(s) {
            Some(i) => DocId(i),
            None => self.text_interned(s),
        }
    }

    /// The interned half of [`Self::text`] for a static outside the prelude: the
    /// slot probe, and the cold miss behind it.
    #[inline]
    fn text_interned(&self, s: &'static str) -> DocId {
        let ptr = s.as_ptr() as usize;
        // Hash in u64: usize is 32-bit on wasm32, where the Fibonacci constant
        // and the top-11-bit shift would overflow. The `>> 53` keeps the top 11
        // bits ⇒ index < 2048, locked to `STATIC_CACHE_SLOTS` by the assert at
        // its definition (and eliding the bounds check below).
        let slot_i = ((ptr as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 53) as usize;
        let slot = &self.static_cache[slot_i];
        if slot.ptr.get() == ptr
            && slot.len.get() as usize == s.len()
            && slot.node_gen.get() == self.format_gen.get()
        {
            return slot.node_id.get();
        }
        self.text_miss(s, slot_i)
    }

    /// The cold half of [`Self::text_interned`]: alloc the node and (re)stamp the slot.
    ///
    /// Reuses the slot's cached width when only the node half is stale (the
    /// once-per-static-per-document case); measures it on a true width miss
    /// (first sighting or collision evict).
    #[cold]
    #[inline(never)]
    fn text_miss(&self, s: &'static str, slot_i: usize) -> DocId {
        let slot = &self.static_cache[slot_i];
        let ptr = s.as_ptr() as usize;
        let width = if slot.ptr.get() == ptr && slot.len.get() as usize == s.len() {
            slot.width.get()
        } else {
            pooled_text_width(s)
        };
        let node_id = self.alloc(DocNode::Text(DocText::Static(s, width)));
        slot.ptr.set(ptr);
        slot.len.set(s.len() as u32);
        slot.width.set(width);
        slot.node_gen.set(self.format_gen.get());
        slot.node_id.set(node_id);
        node_id
    }

    /// Create a text doc from a dynamically-built string, copied into the
    /// arena text pool.
    ///
    /// Takes `&str` — the body is copied into the pool either way, so callers
    /// with a source slice pass it directly (no transient `String`), and
    /// callers that build a `String` pass a borrow and keep (or immediately
    /// drop) their buffer.
    ///
    /// ⚠️ **A slice of the DOCUMENT wants [`Self::source_span`] instead, and whether that
    /// pays is a question of the slice's length.** The two nodes render identically; this
    /// one adds the pool copy (a `RefCell` borrow, a `push_str`, a `memcpy` call) and
    /// measures the width over the bare slice (a scalar tail under eight bytes), where
    /// `source_span` reads the host's word and copies nothing. Measured on the CSS
    /// printer: a ~12-byte selector leaf is ≈ 48 instructions an ask cheaper as a span
    /// (12.3K asks a pass, −0.34% of the format run); a three-byte function name is the
    /// SAME price either way — the copy of three bytes costs what a span's guard costs —
    /// so a short text the caller cannot prove is the span's head stays pooled.
    ///
    /// Width-cache policy: see [`pooled_text_width`] (always eager, so the
    /// fits walk never touches the pool).
    #[inline]
    pub fn text_pooled(&self, s: &str) -> DocId {
        let w = pooled_text_width(s);
        let span = self.pool_push(s);
        self.alloc(DocNode::Text(DocText::Pooled(span, w)))
    }

    /// Create a multi-line text doc rendered with per-line context indent.
    ///
    /// `s`'s lines (split on `\n`) are emitted as: the first at the current
    /// column, each subsequent one after a context-indented hardline. See
    /// [`DocNode::MultilineText`]. Use for indentable multi-line block comments;
    /// the body must already be framed (delimiters + per-line spacing baked in).
    ///
    /// The first line's visual width is precomputed here (clamped like every
    /// cached text width — the fits verdict only compares against print widths
    /// orders of magnitude below the clamp), so fits measures the node without
    /// borrowing the pool.
    #[inline]
    pub fn multiline_text(&self, s: &str) -> DocId {
        self.multiline_text_with(s, false)
    }

    /// [`Self::multiline_text`] whose interior breaks are `literalline`s — a bare
    /// newline, no trim and no indent — for a non-indentable multi-line block
    /// comment, whose interior is kept verbatim. Same framing contract: `s` holds
    /// the delimiters and every line exactly as it should print.
    #[inline]
    pub fn multiline_text_literal(&self, s: &str) -> DocId {
        self.multiline_text_with(s, true)
    }

    fn multiline_text_with(&self, s: &str, literal: bool) -> DocId {
        let first = &s[..next_lf(s.as_bytes(), 0)];
        let first_width = clamp_text_width(visual_width(first, TAB_WIDTH));
        let span = self.pool_push(s);
        self.alloc(DocNode::MultilineText {
            span,
            first_width,
            literal,
        })
    }

    /// Create a pooled-text doc (via [`Self::text_pooled`]) for a *line comment*
    /// (`// …` or hashbang) — text whose content runs to end-of-line.
    ///
    /// Identical to [`Self::text_pooled`] for output. Under the `swallow_check`
    /// feature, while the check is enabled (`super::swallow` — not linked, since
    /// that module only exists under the feature) it additionally
    /// records the node's id so the renderer can flag any content emitted on the
    /// same physical line after it (silent content loss). Without the feature it
    /// is exactly `text_pooled` — no recording, no side-set.
    #[inline]
    pub fn line_comment_text_pooled(&self, s: &str) -> DocId {
        let id = self.text_pooled(s);
        #[cfg(feature = "swallow_check")]
        if swallow_check_enabled() {
            // Recorded in alloc order → sorted ascending (see field doc).
            self.line_comment_ids.borrow_mut().push(id.0);
        }
        id
    }

    /// Start a streaming pooled-text build: assemble a dynamic string piecewise
    /// (no transient caller `String`), then finish into a doc node.
    ///
    /// The writer owns a scratch buffer parked on the arena (`pool_scratch`),
    /// so no pool borrow is held while it is open — interleaved `text_pooled`/
    /// `multiline_text`/nested `pool_writer` calls stay **correct by
    /// construction** (the written bytes enter the shared pool only at
    /// `finish_*`, atomically), not merely non-panicking. Finishing consumes
    /// the writer (`finish_text` / `finish_multiline_text`) and returns the
    /// scratch, capacity retained; a writer dropped unfinished emits nothing
    /// (its buffer, and the capacity it grew, is simply discarded).
    #[inline]
    pub fn pool_writer(&self) -> PoolTextWriter<'_> {
        PoolTextWriter {
            arena: self,
            scratch: self.pool_scratch.take(),
        }
    }

    /// Create a text doc from a verbatim source slice, resolved at render time
    /// against `source` (no `String` allocation). The doc renders byte-identically
    /// to `text_pooled(span.extract(source))` — use it wherever a
    /// printer emits an unmodified source slice (comments, template chunks,
    /// already-canonical literals). `source` is read only to precompute width
    /// (the eager [`pooled_text_width`] policy: a real width or the newline
    /// sentinel, so fits and render never re-scan the text) and is **not**
    /// retained — the span lives in the lifetime-less arena and is re-resolved
    /// at render against the document source threaded through the render entry
    /// points (`resolve_text`). Identifier names come through here too — see the
    /// eager width-cache policy on [`pooled_text_width`] for why the deferral
    /// they once had is gone.
    ///
    /// ⚠️ **`#[inline(never)]` here is a PIN, and on the current shape it is inert** —
    /// removing it produces a byte-identical `.text` and an identical instruction
    /// count. It is kept because LLVM has chosen differently before: when
    /// [`source_span_width`]'s body first shrank, LLVM inlined this function into its
    /// then ~50 callers and outlined the width instead, and that split cost **+0.301%**
    /// of a TypeScript format run; the attribute was worth 0.126 points and −12,352 B
    /// of `.text` against it at the time. Since then the identifier-name population —
    /// three quarters of the calls — moved to [`Self::source_span_plain`], and with the
    /// remaining call sites LLVM outlines this function unprompted. The pin costs nothing
    /// now and holds the shape if a later change re-tempts the other split; re-measure
    /// it, never assume it (an attribute is a rung, not a polish step).
    ///
    /// ⚠️ Two shapes that would remove the call itself were measured on the pre-plain
    /// call population and are still not taken: `#[inline(always)]` here recovered
    /// `instructions:u` −1.20% → −1.51% for **+7,264 B** of `.text` (perf's own inline
    /// threshold is a U-curve this codebase already sits at the minimum of, so pushing
    /// it up is a cycles risk), and a second entry point with a **duplicated** body
    /// reached −1.45% for +336 B but splits one width computation across two sites that
    /// nothing keeps in agreement. A profile-guided build makes this choice per call
    /// site without either cost.
    #[inline(never)]
    pub fn source_span(&self, span: Span, source: &str) -> DocId {
        let w = source_span_width(span, source);
        self.alloc(DocNode::Text(DocText::SourceSpan(span, w)))
    }

    /// [`Self::source_span`] for a span the caller has already PROVED holds no byte
    /// the width depends on — no `\t`, no `\n`, nothing at or above `0x80` — so its
    /// visual width IS its byte length and there is nothing to measure. Renders
    /// byte-identically to [`Self::source_span`]; it takes no `source` at all, which
    /// is the contract stated in the signature.
    ///
    /// ⭐ **The caller this exists for is the identifier name**, and it is the single
    /// largest population of width measures a format run makes: 483,358 of the 640,428
    /// source-span widths on a 1,666-file TypeScript corpus (74% of every width call,
    /// mean 8.05 bytes), of which **three** hold a non-ASCII byte. The proof is not a
    /// scan but a fact the lexer already had: `IdentifierPart`'s ASCII subset is
    /// `[A-Za-z0-9_$]`, so a name that took no non-ASCII branch is one column a byte
    /// and can hold no line terminator or tab — see `IdentName::plain_ascii`.
    ///
    /// ⭐ **The second caller class is the verbatim LITERAL, and it knows for two other
    /// reasons — neither of them a scan of its own.** A number literal is plain ASCII by
    /// grammar (34,567 calls, 22.0% of the non-name width measures on that corpus). A
    /// string literal printed between the quotes it was written with is 54.0% of them
    /// (84,771, of which 83,559 claim), and its quote choice has already read every
    /// content byte looking for a `'` — so `printing::optimal_string_quote_in` hands the
    /// width class out of that one pass for the price of one more lane kernel. Both
    /// arrive through `Printer::verbatim_literal_doc`.
    ///
    /// The CSS printer's value leaves are a third class: an identifier its normalization
    /// pass proved free of any byte below `!` or above `~`, an operator, a unit it scanned,
    /// and a named or hex color, ASCII letters and digits by grammar.
    ///
    /// The Svelte printer's fused constructs are a fourth: a template construct that prints
    /// as its own source bytes — an attribute, an expression tag, or the bare name or lone
    /// lookup that is a braced value or a binding-pattern leaf — emitted as one span over
    /// the whole construct once a scan of it (or the name flags of the path it holds)
    /// proved it plain.
    ///
    /// ⚠️ **A wrong claim here is a SILENT width error.** Nothing downstream re-derives
    /// the width, so an over-claiming caller shifts a fits verdict and changes no other
    /// observable — invisible to a byte-identity sweep on any corpus whose names are
    /// ASCII, which is every corpus. The seam that calls this asserts the property in
    /// debug builds; keep that assert wherever a new caller appears.
    pub fn source_span_plain(&self, span: Span) -> DocId {
        let w = clamp_text_width((span.end - span.start) as usize);
        self.alloc(DocNode::Text(DocText::SourceSpan(span, w)))
    }

    /// [`Self::source_span`] for a **format-ignored verbatim slice** of a
    /// JavaScript-level construct (the `prettier-ignore` freeze): emits
    /// [`DocText::VerbatimSpan`] — identical in measurement and render, but
    /// opaque to `will_break` (full rationale on the variant's doc). Use ONLY
    /// for those slices; a frozen Svelte template node and genuine multi-line
    /// content (line-continuation strings, `<pre>` text) keep
    /// [`Self::source_span`] so they force-break.
    #[inline]
    pub fn verbatim_source_span(&self, span: Span, source: &str) -> DocId {
        let w = source_span_width(span, source);
        self.alloc(DocNode::Text(DocText::VerbatimSpan(span, w)))
    }

    /// Verbatim-source-slice form of [`Self::line_comment_text_pooled`]: emits a
    /// [`DocText::SourceSpan`] (no allocation) and, under the `swallow_check`
    /// feature while enabled, records the node so the renderer can flag content
    /// emitted on the same physical line after a `//`/hashbang comment. Without
    /// the feature it is exactly [`Self::source_span`].
    #[inline]
    pub fn line_comment_source_span(&self, span: Span, source: &str) -> DocId {
        let id = self.source_span(span, source);
        #[cfg(feature = "swallow_check")]
        if swallow_check_enabled() {
            // Recorded in alloc order → sorted ascending (see field doc).
            self.line_comment_ids.borrow_mut().push(id.0);
        }
        id
    }

    /// Whether `id` is a line-comment text node (diagnostic; binary search over
    /// the sorted side-set). Only meaningful while the swallow check is enabled.
    /// Internal to the renderer's swallow check — not part of the builder API.
    #[cfg(feature = "swallow_check")]
    #[inline]
    pub(crate) fn is_line_comment(&self, id: DocId) -> bool {
        self.line_comment_ids.borrow().binary_search(&id.0).is_ok()
    }

    /// Whether `id` is a bare collapsible `Line` separator (`Normal`/`Soft`) — a fill
    /// part that separates from what follows rather than being content itself. The fill
    /// renderer uses it to recognize a *trailing* separator that a leading-separator
    /// parity shift has stranded in the last-item position, so it renders it by fit
    /// (space when it fits, newline when it doesn't) instead of the content path — which
    /// would break to a new line and then render the `Line` flat, stranding a stray
    /// leading space at the head of the continuation.
    #[inline]
    pub(crate) fn is_collapsible_line(&self, id: DocId) -> bool {
        matches!(
            self.nodes.borrow()[id.index()],
            DocNode::Line(LineKind::Normal | LineKind::Soft)
        )
    }

    /// Whether `id` is a **multi-line leaf** — a `Text` holding a newline or a
    /// `MultilineText`: content whose measured width is its FIRST line's and whose
    /// last line is where the column actually is once it has rendered. The fill
    /// renderer asks it of an item that did not fit at line start: such a leaf cannot
    /// break internally and did not wrap, its first line simply overruns, so the item
    /// after it is placed by the pair check from the leaf's real end rather than by the
    /// isolating break a wrapped item takes (`space_separated_multiline_comment_first_line_long`).
    #[inline]
    pub(crate) fn is_multiline_leaf(&self, id: DocId) -> bool {
        match &self.nodes.borrow()[id.index()] {
            DocNode::Text(t) => matches!(t.cached_width(), CachedWidth::HasNewline { .. }),
            DocNode::MultilineText { .. } => true,
            _ => false,
        }
    }

    /// The **breakable atom** `id` contributes to a flow-boundary measurement, if it is one — an
    /// after-element fold's LEAD element (the inline element its trailing text packs after), or a
    /// bare glued element / glued element run ([`DocContext::glued_atom`]). `None` for anything
    /// else — a text run, a plain concat, a bare element carrying no context.
    ///
    /// This is the question asked of the node at the TOP of the look-ahead stack, and unlike
    /// [`Self::welded_entry`] it does NOT ask whether `id` is glued: the top node sits behind the
    /// very boundary being measured, so only its atom shape matters — a fold contributes its lead
    /// whether or not its own head is welded to the text before it (prettier's pairwise fill:
    /// `text, separator, element` — never the trailing tail, which wraps behind the fold's own
    /// `line`).
    #[inline]
    pub(crate) fn welded_atom(&self, id: DocId) -> Option<DocId> {
        let nodes = self.nodes.borrow();
        let DocNode::WithContext { doc, context } = &nodes[id.index()] else {
            return None;
        };
        self.welded_atom_in(&nodes, *doc, context)
    }

    /// The atom-shape half of [`Self::welded_atom`] / [`Self::welded_entry`], under the caller's
    /// node borrow: `doc`/`context` are a `WithContext`'s payload, already destructured.
    #[inline]
    fn welded_atom_in(&self, nodes: &[DocNode], doc: DocId, context: &DocContext) -> Option<DocId> {
        if context.after_element_fold() {
            // The fold is `fill([element, line, words…])`; its breakable atom is the lead element.
            let DocNode::Fill(range) = &nodes[doc.index()] else {
                return None;
            };
            return range.resolve(&self.children.borrow()).first().copied();
        }
        if context.glued_atom() {
            Some(doc)
        } else {
            None
        }
    }

    /// Classify `id` as an entry of the welded-unit walk — the flow-boundary look-ahead
    /// ([`DocContext::break_before_wide_flow`]) deciding how far past the inline element its
    /// **pairwise** measurement reaches. It normally ends at the element, because the next run
    /// owns a whitespace boundary to wrap at; a welded run ([`DocContext::glued_lead`]) owns none,
    /// so it shares the element's line by construction and must share its fit check too. Measuring
    /// the element as if the text fused to its closing tag were free to move packs it onto a line
    /// it does not fit — which is the `inline_break_before_*` / `inline_nbsp_boundary_long` shape.
    ///
    /// The [`WeldedEntry::TextRun`]-vs-[`WeldedEntry::Atom`] split is the whole rule. An atom must
    /// be measured **flat** (its inherited Break mode would let `arena_fits` short-circuit inside
    /// the element's own group and report a fit after the open tag alone). A text run must NOT be
    /// measured flat — forcing it counts words that will wrap anyway and breaks the boundary far
    /// too early, isolating `(<Link …>` in `example (<Link …>see docs</Link>) for details.` (a
    /// run's own first *internal* whitespace is where the measurement stops, since everything
    /// past it wraps there). The walk continues past **either** kind while the next entry is
    /// still glued — a weld can run element, glued text, element (`.w<b>yy</b>.z<i>q</i>`), and
    /// stopping at the first atom lets an early short element strand a later wide one — and the
    /// unit ends at the first entry that is not glued, which sits behind a break opportunity of
    /// its own.
    ///
    /// The two atom sources are asymmetric on purpose. A fold announces itself by its own identity
    /// flag, so its lead needs no extra marking. A bare glued element has nothing to announce, so
    /// the builder marks it ([`DocContext::glued_atom`]) — inert at render, since only a `Fill`
    /// consumes a context there.
    ///
    /// ⚠️ **Atom-ness is NOT recoverable from the node's shape**, and the `Fill`-sniff that looks
    /// like it works is the trap: a single-word text run is a bare `Text` and a prefixed one is a
    /// `Concat`, so "wraps a non-`Fill`" reads `.w` as an atom and ends the walk one node early —
    /// the boundary then never sees the element behind it. Keep the question on the flag.
    #[inline]
    pub(crate) fn welded_entry(&self, id: DocId) -> WeldedEntry {
        let nodes = self.nodes.borrow();
        let DocNode::WithContext { doc, context } = &nodes[id.index()] else {
            return WeldedEntry::NotGlued;
        };
        if !context.glued_lead() {
            return WeldedEntry::NotGlued;
        }
        match self.welded_atom_in(&nodes, *doc, context) {
            Some(atom) => WeldedEntry::Atom(atom),
            None => WeldedEntry::TextRun,
        }
    }

    /// Debug tripwire for the marker-BURIAL hazard, called at the welded walk's `NotGlued` stop
    /// (`flow_lookahead` in `arena_render_fill`): descend the stopped doc's first-structural-child
    /// chain and panic if a [`DocContext::glued_lead`] marker sits inside.
    ///
    /// Every builder that WRAPS a marked doc is a burial hazard: [`Self::welded_entry`] reads only
    /// the top node, so a marker inside a bare wrapper (`group([marked, line])` — the shape a
    /// since-retired sibling join took, and the bug it caused before it re-hoisted the flags)
    /// is invisible, the walk stops one entry short, and the run stands and tears its last element
    /// open instead of travelling. Burial keeps the marker as the wrapper's FIRST structural child;
    /// a legitimately nested marker (a glued boundary in some deeper fill) sits behind a `Fill` or
    /// other content node, where this descent stops — so a hit is a buried marker, not a false
    /// positive. The descent mirrors that shape: a `WithContext` not itself marked → its doc, a
    /// `Group` → its contents, a `Concat` → its first child; anything else ends the chain. The
    /// entry doc itself was just classified `NotGlued`, so a depth-0 marker cannot occur.
    ///
    /// Debug builds only (the standing audits build `--profile corpus`, which compiles this out —
    /// the armed sweep is the debug-mode fixture suite plus a debug-profile audit run).
    #[cfg(debug_assertions)]
    pub(crate) fn debug_check_buried_welded_marker(&self, id: DocId) {
        let nodes = self.nodes.borrow();
        let children = self.children.borrow();
        let mut cur = id;
        loop {
            cur = match &nodes[cur.index()] {
                DocNode::WithContext { doc, context } => {
                    assert!(
                        !context.glued_lead(),
                        "buried welded-run marker: a glued_lead-marked doc is the first \
                         structural child of a NotGlued welded-walk entry — a wrapping builder \
                         hid the marker from welded_entry — re-hoist its flags onto the wrapper"
                    );
                    *doc
                }
                DocNode::Group { contents, .. } => *contents,
                DocNode::Concat(range) => {
                    let Some(&head) = range.resolve(&children).first() else {
                        return;
                    };
                    head
                }
                _ => return,
            };
        }
    }

    /// The **single** constructor of the inline-sibling wrap — `group(concat([line, x]))`, an
    /// inline child led by a collapsible boundary `line` (a space when the fill fits, a break when
    /// it wraps). Every producer routes through here (`push_inline_child_doc` in `tsv_svelte`), and
    /// [`Self::strip_leading_line_group`] is its exact structural inverse, so the two cannot drift:
    /// a `strip_leading_line_group_round_trips` test asserts `strip(inline_sibling_line_group(x)) ==
    /// Some(x)`. Co-locating the pair here is the guard the ~600-line gap between producer and
    /// matcher otherwise lacks (a silent `None` reintroduces the stray-space non-idempotency).
    #[inline]
    pub fn inline_sibling_line_group(&self, x: DocId) -> DocId {
        self.group(self.concat(&[self.line(), x]))
    }

    /// The inline-sibling wrap whose leading `line` carries
    /// [`DocContext::hold_line_after_broken_flow`] — `group([WithContext(line, hold), X])`: the
    /// line renders as a forced break when the immediately preceding flow probe answered yes,
    /// and as the ordinary collapsible boundary otherwise (the renderer's `WithContext` arm reads
    /// the flag off the `Line`). The flag rides INSIDE the wrap rather than on a wrapper around
    /// it, so the wrap measures exactly as [`Self::inline_sibling_line_group`] does — a fits walk
    /// descends through `WithContext` and sees an ordinary line — and so that
    /// [`Self::strip_leading_line_group_ex`] still matches it and can report which form it found.
    /// Built by `push_inline_child_doc` in `tsv_svelte` for `LeadBoundary::SpacedHeld`, the
    /// layout-keyed sibling boundary.
    #[inline]
    pub fn inline_sibling_line_group_held(&self, x: DocId) -> DocId {
        let held_line = self.with_context(
            self.line(),
            DocContext::default().with_hold_line_after_broken_flow(true),
        );
        self.group(self.concat(&[held_line, x]))
    }

    /// `id` as a `Fill`, wrapping it in a one-part fill when it is not already one.
    ///
    /// A [`DocContext`] carrying a **render-side** flag ([`DocContext::break_before_wide_flow`],
    /// [`DocContext::glued_lead`]) is only ever read off a `Fill`: `WithContext`'s render arm
    /// dispatches to the fill loop and otherwise descends, dropping the context on the floor. So a
    /// builder attaching one to a non-`Fill` doc silently loses it — and the loss is invisible,
    /// since the flag's whole job is to *change* a boundary decision that still produces valid,
    /// idempotent output without it.
    ///
    /// That is not hypothetical: a text run of a single word is a bare `Text` (a glued-prefixed one
    /// a `Concat`), the same shape-vs-flag trap [`Self::welded_entry`] warns about on the walk side.
    /// A one-word run glued to a following element is exactly where the flow boundary matters most,
    /// and its flag reached no reader at all.
    ///
    /// A one-part fill is the faithful spelling rather than a workaround: the run *is* a fill of one
    /// item, and the fill loop's last-item case (Case 1) is what asks the boundary question.
    #[inline]
    pub fn as_fill(&self, id: DocId) -> DocId {
        if matches!(self.nodes.borrow()[id.index()], DocNode::Fill(_)) {
            return id;
        }
        self.fill(&[id])
    }

    /// If `id` is exactly the inline-sibling wrap [`Self::inline_sibling_line_group`] builds — a
    /// non-breaking `Group(Concat([Line(Normal|Soft), X]))` with no conditional-group states —
    /// return the inner `X` (the bare element doc), dropping the leading boundary `Line`. `None` for
    /// any other shape. The exact inverse of that constructor (round-trip-tested); keep them in
    /// lockstep.
    ///
    /// The after-element fold uses this to keep `X` bare in the fold's lead content slot and
    /// hoist the boundary line OUTSIDE the fold. Otherwise the fill's break and the group's own
    /// re-decided leading line both charge the one boundary — a stray leading space on the
    /// continuation line, which the next pass reads as indentation and drops (non-idempotent).
    #[inline]
    pub fn strip_leading_line_group(&self, id: DocId) -> Option<DocId> {
        self.strip_leading_line_group_ex(id).map(|(x, _)| x)
    }

    /// [`Self::strip_leading_line_group`] over both wrap forms: returns the inner `X` and whether
    /// the wrap was the HELD one ([`Self::inline_sibling_line_group_held`], its leading line
    /// hold-flagged), so a caller that strips, rebuilds around `X` and re-wraps can reproduce
    /// the lead it found — dropping the hold on a rejoin would silently un-hold a boundary the
    /// author wrote. Round-trip-tested against both producers.
    #[inline]
    pub fn strip_leading_line_group_ex(&self, id: DocId) -> Option<(DocId, bool)> {
        let nodes = self.nodes.borrow();
        let DocNode::Group {
            contents,
            expanded_states,
            should_break,
            ..
        } = &nodes[id.index()]
        else {
            return None;
        };
        if *should_break {
            return None;
        }
        let children = self.children.borrow();
        if !expanded_states.resolve(&children).is_empty() {
            return None;
        }
        let DocNode::Concat(range) = &nodes[contents.index()] else {
            return None;
        };
        let [first, x] = range.resolve(&children) else {
            return None;
        };
        let held = match &nodes[first.index()] {
            DocNode::Line(LineKind::Normal | LineKind::Soft) => false,
            DocNode::WithContext { doc, context }
                if context.hold_line_after_broken_flow()
                    && matches!(
                        nodes[doc.index()],
                        DocNode::Line(LineKind::Normal | LineKind::Soft)
                    ) =>
            {
                true
            }
            _ => return None,
        };
        Some((*x, held))
    }

    /// Tag `id` as the doc node that emits the comment at `span` in `source`.
    ///
    /// The print-once comment ledger's build-side seam for a doc-based printer: the
    /// *renderer* records the emit when it reaches this node, so a comment assembled into
    /// a `conditional_group` candidate that loses never counts (and one assembled only
    /// into a losing candidate is correctly reported as dropped). `source` is captured as
    /// a [`DocumentKey`] because the renderer holds no source of its own — the arena is
    /// shared across a Svelte host and a nested element's re-parsed island, whose spans
    /// live in different namespaces. A no-op unless the ledger is enabled, and compiled
    /// out entirely without the `comment_check` feature. See [`crate::comment_ledger`].
    #[cfg(feature = "comment_check")]
    #[inline]
    pub fn tag_comment_doc(&self, id: DocId, span: Span, source: &str) {
        if comment_check_enabled() {
            // Recorded in alloc order → sorted ascending (see field doc). Every comment
            // doc's root is a fresh alloc (a `SourceSpan` / `MultilineText` leaf or a
            // `Concat` container — none of them interned), so ids strictly increase.
            self.comment_docs
                .borrow_mut()
                .push((id.0, span, document_key(source)));
        }
    }

    /// Whether any comment-doc tags were recorded — the renderer's hoisted gate, so a
    /// document with no comments pays no per-node lookup.
    #[cfg(feature = "comment_check")]
    #[inline]
    pub(crate) fn has_comment_docs(&self) -> bool {
        !self.comment_docs.borrow().is_empty()
    }

    /// The comment a doc node emits, if it is one (binary search over the sorted
    /// side-set). Internal to the renderer's ledger hook — not part of the builder API.
    #[cfg(feature = "comment_check")]
    #[inline]
    pub(crate) fn comment_doc_tag(&self, id: DocId) -> Option<(Span, DocumentKey)> {
        let tags = self.comment_docs.borrow();
        let idx = tags
            .binary_search_by_key(&id.0, |&(node, _, _)| node)
            .ok()?;
        let (_, span, key) = tags[idx];
        Some((span, key))
    }

    /// Carry `old`'s comment-doc tag (if any) onto the freshly-allocated `new`.
    ///
    /// A doc-tree *transform* — [`Self::remove_lines`] / [`Self::atomize`] — allocates
    /// a new [`DocId`] for every non-leaf node it rebuilds, including a multi-line block
    /// comment's `Concat` (and, when dropping hard lines, a `MultilineText`). The renderer
    /// records a comment's emit when it reaches the *tagged* node, so a re-allocated comment
    /// doc whose tag stayed on the discarded original would read as **DROPPED** even though it
    /// prints verbatim (the instrument false-positive [`Self::tag_comment_doc`] can't see, because
    /// nothing walks the transform). This copies the tag across the rebuild.
    ///
    /// Sound for the binary-search invariant: the only nodes ever tagged are comment doc roots
    /// (a `SourceSpan` text — left untouched by the transform, so it never reaches here — a
    /// `MultilineText`, or a multi-child `Concat`), and both re-allocated kinds are replaced by
    /// a **fresh** allocation, never an interned/short-circuited id — so whenever this pushes,
    /// `new` is the highest id so far and `comment_docs` stays sorted ascending (see the field
    /// doc + [`Self::tag_comment_doc`]). Safe against double-counting: the transform returns the new
    /// tree and discards the old, and the renderer only records emits for nodes it actually
    /// reaches (a discarded/losing subtree never does), so the old tag left in place never
    /// fires. A no-op unless the ledger is enabled and `old` was tagged; compiled out entirely
    /// without the `comment_check` feature. See [`crate::comment_ledger`].
    #[cfg(feature = "comment_check")]
    #[inline]
    pub(crate) fn retag_comment_doc(&self, new: DocId, old: DocId) {
        if !comment_check_enabled() {
            return;
        }
        if let Some((span, key)) = self.comment_doc_tag(old) {
            let mut tags = self.comment_docs.borrow_mut();
            debug_assert!(
                tags.last().is_none_or(|&(last, ..)| new.0 > last),
                "retag_comment_doc must keep comment_docs sorted ascending on the node id"
            );
            tags.push((new.0, span, key));
        }
    }

    /// Create an empty doc that produces no output — a prelude singleton at a
    /// fixed id, so the call is a constant (see [`PRELUDE`]).
    #[inline]
    pub fn empty(&self) -> DocId {
        DocId(PRELUDE_EMPTY)
    }

    /// Create a normal line break (space if fits, newline if doesn't) — a
    /// prelude singleton at a fixed id (see [`PRELUDE`]).
    #[inline]
    pub fn line(&self) -> DocId {
        self.line_node(LineKind::Normal)
    }

    /// Create a soft line that disappears in flat mode — a prelude singleton.
    #[inline]
    pub fn softline(&self) -> DocId {
        self.line_node(LineKind::Soft)
    }

    /// Create a hard line break (always breaks) — a prelude singleton.
    #[inline]
    pub fn hardline(&self) -> DocId {
        self.line_node(LineKind::Hard)
    }

    /// Create a literal line break (just newline, no indentation) — a prelude
    /// singleton.
    #[inline]
    pub fn literalline(&self) -> DocId {
        self.line_node(LineKind::Literal)
    }

    /// The prelude's `Line` node for `kind`: a `Line` node carries no per-use
    /// state (mode and indent are supplied per visit by the enclosing render
    /// command), so every line of a kind within one document shares one node —
    /// the layout analog of "statics are position-free" — and at a fixed id
    /// (see [`PRELUDE`]) the call is a constant, indexed by the kind's
    /// discriminant.
    #[inline]
    fn line_node(&self, kind: LineKind) -> DocId {
        DocId(PRELUDE_LINE + kind as u32)
    }

    //
    // Structural builders
    //

    /// Create a group (try to fit on one line, break all if doesn't fit).
    pub fn group(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::Group {
            contents: doc,
            expanded_states: ChildRange::EMPTY,
            keyed: false,
            should_break: false,
        })
    }

    /// Create a group that forces break mode during rendering.
    pub fn group_break(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::Group {
            contents: doc,
            expanded_states: ChildRange::EMPTY,
            keyed: false,
            should_break: true,
        })
    }

    /// Create a keyed group — prettier's `group(…, { id })` — and return its id, whose
    /// [`GroupId::doc`] is the node to place in the tree. Readers built with the id
    /// ([`Self::if_break_with_id`], [`Self::indent_if_break`]) read the mode THIS group
    /// resolved to, whatever other keyed groups render between the two.
    pub fn group_with_id(&self, doc: DocId) -> GroupId {
        self.group_with_id_break(doc, false)
    }

    /// [`Self::group_with_id`] whose break mode is decided by the caller rather than by
    /// fit — prettier's `group(…, { id, shouldBreak })`. The id and the forced break go
    /// together whenever some other doc reads the decision through
    /// [`Self::indent_if_break`]: a caller that emitted [`Self::group_break`] instead would
    /// break the group but leave the reader unable to see it.
    pub fn group_with_id_break(&self, doc: DocId, should_break: bool) -> GroupId {
        GroupId::of(self.alloc(DocNode::Group {
            contents: doc,
            expanded_states: ChildRange::EMPTY,
            keyed: true,
            should_break,
        }))
    }

    /// Create a conditional group that tries multiple alternative layouts.
    ///
    /// `states[0]` is tried first (stored as `contents`), `states[1..]` stored in `expanded_states`.
    pub fn conditional_group(&self, states: &[DocId]) -> DocId {
        assert!(
            !states.is_empty(),
            "conditional_group requires at least one state"
        );
        let first = states[0];
        let expanded = self.alloc_children(&states[1..]);
        self.alloc(DocNode::Group {
            contents: first,
            expanded_states: expanded,
            keyed: false,
            should_break: false,
        })
    }

    /// Create a conditional-group state admitted only while `probe` cannot fit
    /// flat on a fresh line one indent level deeper than the enclosing
    /// conditional group — see [`DocNode::GatedState`].
    pub fn gated_state(&self, probe: DocId, contents: DocId) -> DocId {
        self.gated_state_admitting(probe, contents, GateAdmit::ProbeOverflows)
    }

    /// A [`DocNode::GatedState`] probe standing for the line `doc` heads
    /// `extra_indents` levels deeper than the state's own fresh line, and ending
    /// where `doc`'s first line does.
    ///
    /// The gate measures its probe one indent level deeper than the group,
    /// completed by the group's rest commands. Two things a state's real line
    /// can differ in from that: it sits deeper (a member chain's broken-out
    /// argument is two levels below the chain), and it ENDS where the rest
    /// commands would continue it (a broken-out argument's line stops before
    /// the `)` and `;` the rest supply). The prefix is `TAB_WIDTH` columns per
    /// level — exactly a tab's width, which is all an indent level is — and the
    /// trailing hard line ends the fits walk there, fitting, so no rest command
    /// is charged. Measured only: no walk but the gate's admission reads a
    /// probe, so neither the padding nor the hard line ever renders.
    pub fn line_probe(&self, extra_indents: usize, doc: DocId) -> DocId {
        const PAD: &str = "        ";
        let width = TAB_WIDTH * extra_indents;
        assert!(
            width <= PAD.len(),
            "line_probe: {extra_indents} extra indents"
        );
        let line = [self.text(&PAD[..width]), doc, self.hardline()];
        self.concat(if width == 0 { &line[1..] } else { &line })
    }

    /// Create a conditional-group state admitted only while `probe` FITS flat on a
    /// fresh line one indent level deeper than the enclosing conditional group —
    /// see [`DocNode::GatedState`].
    pub fn gated_state_when_fits(&self, probe: DocId, contents: DocId) -> DocId {
        self.gated_state_admitting(probe, contents, GateAdmit::ProbeFits)
    }

    /// Create a conditional group's FINAL state that only an enclosing look-ahead
    /// reads: the render never prints it, taking the state before it when no
    /// earlier state fits — see [`GateAdmit::Never`]. The group needs at least two
    /// other states.
    pub fn measure_only_state(&self, contents: DocId) -> DocId {
        self.gated_state_admitting(contents, contents, GateAdmit::Never)
    }

    fn gated_state_admitting(&self, probe: DocId, contents: DocId, admit: GateAdmit) -> DocId {
        self.alloc(DocNode::GatedState {
            probe,
            contents,
            admit,
        })
    }

    /// Increase indentation for nested doc.
    pub fn indent(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::Indent(doc))
    }

    /// Decrease indentation for doc.
    pub fn dedent(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::Dedent(doc))
    }

    /// Reset `doc` to an absolute whole-tab indentation level (the
    /// template-literal root reset — see [`DocNode::AlignRoot`]).
    pub fn align_root(&self, n: usize, doc: DocId) -> DocId {
        self.alloc(DocNode::AlignRoot { n, contents: doc })
    }

    /// Offset `doc` by a sub-tab alignment of `n` literal spaces — Prettier's
    /// numeric `align(n, …)`. Under `useTabs` this renders as spaces at a
    /// trailing (line-ending) position and rounds up to a whole tab when a
    /// further `indent` is stacked on it — see [`DocNode::Align`].
    pub fn align(&self, n: u32, doc: DocId) -> DocId {
        self.alloc(DocNode::Align { n, contents: doc })
    }

    /// Conditional rendering based on parent group breaking.
    pub fn if_break(&self, break_doc: DocId, flat_doc: DocId) -> DocId {
        self.alloc(DocNode::IfBreak {
            break_doc,
            flat_doc,
            group_id: None,
        })
    }

    /// Conditional rendering based on whether one specific group broke.
    ///
    /// Unlike `if_break`, which keys on the immediately enclosing group, this
    /// keys on the resolved mode of the group `group_id` names — so it can sit
    /// outside the group it reacts to (e.g. a block-tag head's `}` after its head
    /// group). During `fits()` the keyed group is treated as unresolved (flat), so
    /// trailing text after the conditional is still counted toward the group's own
    /// break decision (the `}` stays in the head's width).
    pub fn if_break_with_id(&self, break_doc: DocId, flat_doc: DocId, group_id: GroupId) -> DocId {
        self.alloc(DocNode::IfBreak {
            break_doc,
            flat_doc,
            group_id: Some(group_id),
        })
    }

    /// Conditionally indent based on whether the group `group_id` names broke.
    pub fn indent_if_break(&self, doc: DocId, group_id: GroupId) -> DocId {
        self.alloc(DocNode::IndentIfBreak {
            contents: doc,
            group_id,
        })
    }

    /// Concatenate multiple docs into a sequence.
    ///
    /// Short-circuits the degenerate cases so no `Concat` node is allocated for
    /// them: an empty slice returns `empty()` (a `Concat` with no children emits
    /// nothing, exactly like `empty()`), and a single element returns that
    /// element's `DocId` directly — `concat([x])` renders exactly as `x`, since
    /// every consumer of `Concat` only resolves and iterates its child range, so
    /// wrapping one child changes no output, `fits()` result, or break decision.
    /// These two shapes are ~7% of all doc nodes on real corpora (single-child
    /// alone ~6%), so collapsing them at this chokepoint cuts build allocation,
    /// arena memory, and the render/`fits`/memo traversal that scans every node.
    ///
    /// Nested-`Concat` splicing (copying a `Concat` part's children inline —
    /// associativity makes it output-identical) was prototyped and measured a
    /// net instruction regression (+0.2–0.5% on 4 of 6 corpora): the per-part
    /// node-kind check runs on every child slot while the savings accrue only
    /// on nested nodes, and inner concats average ~6 children, so the children
    /// vec grew +78%. Don't re-attempt without a new idea.
    ///
    /// ⚠️ **The head is `inline(always)` and the two allocating arms are
    /// `inline(never)`, and the split is what makes that affordable.** This is
    /// the doc builders' widest chokepoint — over 1,200 call sites, most of
    /// them a literal array — so a call here is a stack array written, a
    /// length passed, a length re-dispatched, and five callee-saved registers
    /// spilled for cold edges the hot path never takes. Forcing the *whole*
    /// body in erases all of that and measures `instructions:u` **−0.89%** on
    /// `fuz_app`, but it copies the allocation and its grow/borrow edges to
    /// every site: **+349 KB of `.text` (+12.1%)**. Keeping the head alone
    /// inline costs **+9.7 KB** of `.text` and **+13.7 KB** of the `format`
    /// WASM bundle — the `parse` bundle is byte-identical, since it links no
    /// doc builder at all.
    ///
    /// ⚠️ **The split is right and the reason first recorded for it was not.**
    /// It was landed on an `instructions:u` win of −0.40…−0.50%, measured
    /// before the static-node cache was sized against its collision draw (see
    /// [`STATIC_CACHE_SLOTS`]) — i.e. inside a 0.4–0.6% per-exec lottery of
    /// exactly that magnitude. Re-measured on the fixed instrument, against the
    /// un-split spelling built four times over, the split reads **+0.108%
    /// instructions** — the opposite sign — for **−167 KB** of `.text`. The
    /// un-split body is small enough that LLVM folds it whole at the
    /// literal-array sites and constant-folds the dispatch, which is where its
    /// instructions go and why its i-cache footprint explodes. **The split
    /// stands on the 167 KB**, not on the instruction count, which reads it
    /// backwards.
    ///
    /// ⚠️ The same re-grade also recorded **−1.29% cycles / −1.2% wall**, and
    /// that half does *not* stand: those four builds a side were perturbed by a
    /// constant in [`Self::text`]'s slot hash, which leaves code layout
    /// untouched, so both groups sampled one layout apiece and the figure is a
    /// layout draw of the size the effect was claimed to be. Re-measured with a
    /// layout-sampling group it would be worth having; until then treat this
    /// chokepoint as **cycles-unmeasured**. See `specialize_short_len!`'s ladder
    /// note and `docs/performance.md` §Reading `cycles:u`.
    ///
    /// The two-child arm earns its own entry point because a child range holds
    /// exactly two ids in 65% of calls (see [`Self::alloc_children`]): taking
    /// them by value makes the folded call site a register handoff with no
    /// array to materialize.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    #[cfg_attr(feature = "census", track_caller)]
    pub fn concat(&self, docs: &[DocId]) -> DocId {
        #[cfg(feature = "census")]
        crate::census::site(core::panic::Location::caller(), docs.len());
        match docs {
            [single] => *single,
            [a, b] => self.concat_pair(*a, *b),
            _ => self.concat_other(docs),
        }
    }

    /// [`Self::concat`] over parts produced one at a time — a chain group's node docs,
    /// a flattened child list, a first doc ahead of its rest. Three quarters of the
    /// buffers the printers assembled for a `concat` held one or two parts (a census
    /// over real TypeScript: of 894 K buffer-built concats a pass, 674 K were one- or
    /// two-part), and a one-part concat IS its part — so the first three parts are
    /// pulled before any buffer exists: none → `empty()`, one → itself, two →
    /// [`Self::concat_pair`], and only a third opens a [`DocBuf`] for the rest.
    ///
    /// Parts are pulled in order, so an iterator that builds docs as it goes allocates
    /// exactly as the `collect` it replaces did — the doc tree is the same node for node.
    #[inline]
    pub fn concat_iter(&self, parts: impl IntoIterator<Item = DocId>) -> DocId {
        let mut parts = parts.into_iter();
        let Some(a) = parts.next() else {
            return self.empty();
        };
        let Some(b) = parts.next() else {
            return a;
        };
        let Some(c) = parts.next() else {
            return self.concat_pair(a, b);
        };
        self.concat_from_three(a, b, c, &mut parts)
    }

    /// [`Self::concat_iter`]'s three-or-more arm, out of line and over a `dyn` iterator
    /// so every `concat_iter` instantiation shares one body: the buffer, its spill and
    /// the `extend` stay out of the callers' frames, and only the pull of the first
    /// three parts is inlined. The rest is still pulled in order.
    #[inline(never)]
    fn concat_from_three(
        &self,
        a: DocId,
        b: DocId,
        c: DocId,
        rest: &mut dyn Iterator<Item = DocId>,
    ) -> DocId {
        let mut buf = DocBuf::new();
        buf.push(a);
        buf.push(b);
        buf.push(c);
        buf.extend(rest);
        self.concat_other(&buf)
    }

    /// [`Self::concat`]'s two-child arm, taking its children by value so the
    /// inlined head is a register handoff rather than a stack array.
    ///
    /// ⚠️ **The `inline(never)` is a guard, not a measured win — keep it
    /// anyway.** Dropping it is `.text`-neutral to the byte: the cost model
    /// declines to fold this body in on its own, the same refusal that left
    /// `concat` out of line to begin with. But "the head is forced in *because*
    /// the arms are not" is the whole design, the guard is free, and the
    /// failure mode if that refusal ever changes is the +349 KB build the
    /// head's doc describes. Same for [`Self::concat_other`].
    #[inline(never)]
    fn concat_pair(&self, a: DocId, b: DocId) -> DocId {
        let range = self.alloc_children(&[a, b]);
        self.alloc(DocNode::Concat(range))
    }

    /// [`Self::concat`]'s remaining arms — the empty short-circuit and three or
    /// more children — kept out of line so a call site pays only the head.
    #[inline(never)]
    fn concat_other(&self, docs: &[DocId]) -> DocId {
        if docs.is_empty() {
            return self.empty();
        }
        let range = self.alloc_children(docs);
        self.alloc(DocNode::Concat(range))
    }

    /// Create a fill doc for greedy line packing.
    #[cfg_attr(feature = "census", track_caller)]
    pub fn fill(&self, parts: &[DocId]) -> DocId {
        #[cfg(feature = "census")]
        crate::census::site(core::panic::Location::caller(), parts.len());
        let range = self.alloc_children(parts);
        self.alloc(DocNode::Fill(range))
    }

    /// Wrap a doc with rendering context.
    ///
    /// ⚠️ A `DocContext` reaches the RENDERER only through a `Fill`: `WithContext`'s render arm
    /// dispatches to the fill loop and otherwise just descends, dropping the context. The debug
    /// tripwire below holds the two flags that are purely render-side to that channel — see
    /// [`Self::as_fill`], which is how a builder satisfies it for a run that isn't already a fill.
    ///
    /// The other flags are deliberately exempt, because they are read at BUILD time off whatever
    /// shape they mark: [`DocContext::glued_lead`] and [`DocContext::glued_atom`] mark an element
    /// doc for the welded walk ([`Self::welded_entry`]), and [`DocContext::trailing_reserve`] marks
    /// a bare `empty()` in the CSS printer. Asserting on those would fire on correct code.
    pub fn with_context(&self, doc: DocId, context: DocContext) -> DocId {
        #[cfg(debug_assertions)]
        debug_assert!(
            !(context.break_before_wide_flow() || context.after_element_fold())
                || matches!(self.nodes.borrow()[doc.index()], DocNode::Fill(_)),
            "render-side DocContext flag on a non-Fill doc — the renderer will drop it silently \
             (wrap with DocArena::as_fill)"
        );
        self.alloc(DocNode::WithContext { doc, context })
    }

    /// Content to print at the end of the current line.
    pub fn line_suffix(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::LineSuffix(doc))
    }

    /// Force pending LineSuffix content to be flushed — a prelude singleton
    /// (stateless, like [`Self::line`]: one shared node per document, at a fixed id).
    #[inline]
    pub fn line_suffix_boundary(&self) -> DocId {
        DocId(PRELUDE_LINE_SUFFIX_BOUNDARY)
    }

    /// The end of an embedded document ([`DocNode::EmbedEnd`]): a pending
    /// [`Self::line_suffix`] flushes here and ends the line, since past this point the line
    /// is the host's.
    ///
    /// `run` is the pending run's static shape ([`Self::pending_run_at_end`]); `None` — a
    /// run the static reading did not see — measures as a zero-width run that breaks.
    /// `dedent_closer` puts the break that ends the run one level out from the node's own
    /// indent: the closer that follows sits outside an indent the value is in.
    ///
    /// The returned id names the node ([`GroupId::doc`] is the doc to place) and keys what
    /// the render found there: an [`Self::if_break_with_id`] on it takes its break arm
    /// exactly when a run flushed at this node — when the value, as rendered, ends on a
    /// comment (a host's terminator that must stand between a comment and its closer).
    pub fn embed_end(&self, run: Option<PendingRun>, dedent_closer: bool) -> GroupId {
        let (run_width, run_breaks) = run.map_or((0, true), |r| (r.width, r.has_line_comment));
        GroupId::of(self.alloc(DocNode::EmbedEnd {
            run_width,
            run_breaks,
            dedent_closer,
        }))
    }

    /// Whether `id`'s last printed leaf can be a COMMENT — a text that opens `/*` or `//`,
    /// or a multi-line text (a block comment's reprint) — in some layout, as opposed to
    /// code. Deferred runs ([`DocNode::LineSuffix`]) are not leaves here: where they flush
    /// is [`Self::pending_run_at_end`]'s question. A static reading, conservative toward
    /// "yes": each alternative (an `if_break` arm, a conditional group's state) is read,
    /// and one ending on a comment answers for all. Each node's effect on the reading is read
    /// once a call ([`LastLeafStep`]), however many alternatives share it.
    pub fn may_end_on_comment_text(&self, id: DocId, source: &str) -> bool {
        let nodes = self.nodes.borrow();
        let children = self.children.borrow();
        let pool = self.text_pool.borrow();
        let store = WalkStore {
            nodes: &nodes,
            children: &children,
            pool: &pool,
            source,
        };
        // the reading ahead of `id` is `Nothing`, so only what `id` itself prints answers
        store.last_leaf(id, &mut WalkMemo::new()).found == LastLeaf::Comment
    }

    /// Whether `id`'s subtree holds a [`DocNode::LineSuffix`] — a deferred run that may
    /// still be pending when the subtree ends. A plain walk, for a caller that has already
    /// established the subtree holds a comment at all.
    ///
    /// The answer is reachability, so a node already expanded need not be again: past
    /// [`WALK_MEMO_AFTER`] visits the walk keeps the nodes it has expanded and skips them,
    /// which bounds it by the nodes it can reach however much of the doc is shared.
    pub fn holds_line_suffix(&self, id: DocId) -> bool {
        let nodes = self.nodes.borrow();
        let children = self.children.borrow();
        let mut stack: SmallVec<[DocId; 16]> = SmallVec::new();
        let mut expanded: WalkMemo<DocId, ()> = WalkMemo::new();
        stack.push(id);
        while let Some(id) = stack.pop() {
            crate::fanout_probe::walk();
            if expanded.visit(id).is_some() {
                continue;
            }
            expanded.record(id, ());
            match &nodes[id.index()] {
                DocNode::LineSuffix(_) => return true,
                DocNode::Indent(inner) | DocNode::Dedent(inner) => stack.push(*inner),
                DocNode::AlignRoot { contents, .. }
                | DocNode::Align { contents, .. }
                | DocNode::IndentIfBreak { contents, .. } => stack.push(*contents),
                DocNode::GatedState { contents, .. } => stack.push(*contents),
                DocNode::Group {
                    contents,
                    expanded_states,
                    ..
                } => {
                    stack.push(*contents);
                    stack.extend_from_slice(expanded_states.resolve(&children));
                }
                DocNode::IfBreak {
                    break_doc,
                    flat_doc,
                    ..
                } => {
                    stack.push(*break_doc);
                    stack.push(*flat_doc);
                }
                DocNode::Concat(range) | DocNode::Fill(range) => {
                    stack.extend_from_slice(range.resolve(&children));
                }
                DocNode::WithContext { doc, .. } => stack.push(*doc),
                DocNode::Text(_)
                | DocNode::MultilineText { .. }
                | DocNode::Line(_)
                | DocNode::LineSuffixBoundary
                | DocNode::EmbedEnd { .. }
                | DocNode::BreakParent
                | DocNode::FlushBreak
                | DocNode::FlowProbeEnd => {}
            }
        }
        false
    }

    /// The deferred run that may still be PENDING when `id` ends — `None` when every
    /// [`DocNode::LineSuffix`] in it meets a line end the doc guarantees first.
    ///
    /// A static reading of the render's flush points, in emission order: a hard line (and a
    /// multi-line text, which is hard lines), a [`DocNode::LineSuffixBoundary`], and a soft
    /// or normal line behind a [`DocNode::FlushBreak`] (which exists to make that line break)
    /// all flush the run; any other line is width-dependent and is read as not breaking. An
    /// `if_break`'s two arms and a conditional group's states are each read, and joined: a
    /// run pending in any of them is pending, and `certain` only when pending in all. The run's shape is read as an [`DocNode::EmbedEnd`] flushes
    /// it — each comment behind no `//` glued onto the line (see
    /// [`PendingRun`]) — which is also how a host lays out the same comments written after
    /// its value, the reason a host asks. A subtree the alternatives share is read once a
    /// call per state the walk reaches it in.
    pub fn pending_run_at_end(&self, id: DocId, source: &str) -> Option<PendingRun> {
        let nodes = self.nodes.borrow();
        let children = self.children.borrow();
        let pool = self.text_pool.borrow();
        let store = WalkStore {
            nodes: &nodes,
            children: &children,
            pool: &pool,
            source,
        };
        let state = store.suffix_walk(id, SuffixWalk::default(), &mut WalkMemo::new());
        let RunShape {
            width,
            has_line_comment,
            ends_in_line_comment,
        } = state.run;
        state.pending().then_some(PendingRun {
            width,
            has_line_comment,
            ends_in_line_comment,
            certain: state.queue == RunQueue::Pending,
        })
    }

    /// What `id` prints ahead of its first code token, in any layout — the reading a
    /// restricted production needs of its argument (`return [no LineTerminator here]
    /// Expression`) and a keyword→value seam of its value, taken from the doc as built
    /// rather than re-derived from the source, so every emitter that can put a comment
    /// there answers alike.
    ///
    /// A comment is recognized by its text: a `/*` or `//` opens no code token. The walk
    /// follows each layout in print order up to the first other text: an `if_break` forks
    /// unless its group's mode is settled (forced broken by its content, flat inside a
    /// flat group, or chosen by an earlier fork of the same layout), and a conditional
    /// group's states each fork. A breakable line whose group is still open ends one
    /// layout as [`LeadingBreak::Soft`] while the flat one goes on. The answer is the worst
    /// over the layouts; a walk that outruns [`LEADING_EDGE_BUDGET`] answers
    /// [`LeadingBreak::Hard`].
    ///
    /// A deferred run ([`DocNode::LineSuffix`]) counts as a comment printed ahead: it
    /// lands before the line end a following line makes.
    pub fn leading_comment_break(&self, id: DocId, source: &str) -> LeadingEdge {
        let nodes = self.nodes.borrow();
        let children = self.children.borrow();
        let pool = self.text_pool.borrow();
        let mut walk = LeadingEdgeWalk {
            arena: self,
            nodes: &nodes,
            children: &children,
            pool: &pool,
            source,
            frames: SmallVec::new(),
            modes: SmallVec::new(),
            layouts: SmallVec::new(),
            edge: LeadingEdge::NONE,
            budget: LEADING_EDGE_BUDGET,
        };
        let next = walk.frame(id, EDGE_OUTSIDE, EDGE_NIL);
        walk.layouts.push(EdgeLayout {
            next,
            modes: EDGE_NIL,
            comment: false,
            suffix: false,
            flush: false,
        });
        walk.run();
        walk.edge
    }

    /// Whether `id` prints a comment on a line of its own in the gap ahead of the code token
    /// that is the source slice starting at `token_start` — the question a restricted
    /// production asks of a member gap its source shows an own-line comment in, since only a
    /// gap the chain prints that way is one the reparse hangs the argument for.
    ///
    /// The gap is the run of non-code text (whitespace, comments, lines) ahead of the token,
    /// together with the run ahead of the code token before it, so a member's `.` / `?.`
    /// printed as a token of its own does not end it. Nothing outside it counts — a line the
    /// doc prints at another gap — and a deferred run ([`DocNode::LineSuffix`]) is read in
    /// place, where it is built, rather than at the end of the line it lands on. Every
    /// layout is followed (each `if_break` arm, a conditional group's states) and every line
    /// counts as broken, so the answer is [`GapCommentLine::OwnLine`] when any layout can
    /// print such a comment; a multi-line block's interior is a line only for the comments
    /// after it, which then stand on the line it began.
    pub fn gap_comment_line(&self, id: DocId, token_start: u32, source: &str) -> GapCommentLine {
        let nodes = self.nodes.borrow();
        let children = self.children.borrow();
        let pool = self.text_pool.borrow();
        let mut walk = GapLineWalk {
            nodes: &nodes,
            children: &children,
            pool: &pool,
            source,
            target: token_start,
            memo: FxHashMap::default(),
            budget: GAP_LINE_BUDGET,
            found_inline: false,
            found_own: false,
        };
        walk.walk_one(id, 0);
        if walk.budget == 0 {
            GapCommentLine::Unknown
        } else if walk.found_own {
            GapCommentLine::OwnLine
        } else if walk.found_inline {
            GapCommentLine::Inline
        } else {
            GapCommentLine::Unknown
        }
    }

    /// `id` with every breakable line its leading edge prints BEHIND A COMMENT replaced by
    /// a space, in every layout it reaches — the separator a keyword→value gap gives a glued
    /// single-line block (`export default /* d */⏎v` prints `export default /* d */ v`),
    /// for a run a value's own printer hoisted to its head and separated by a line inside
    /// the value's group. Only the path down to each replaced line is copied, every subtree
    /// off it shared.
    ///
    /// The layouts it reaches are each `if_break`'s two arms and a group's contents; a
    /// conditional group is walked through its first state alone (`contents`), so a line
    /// only a later state prints is neither replaced nor read.
    ///
    /// `None` — the caller keeps its other layout — when the edge cannot be glued that way:
    /// a hard line, a `//` or a multi-line text ahead of the first token, a
    /// [`DocNode::FlushBreak`], [`DocNode::LineSuffixBoundary`] or [`DocNode::EmbedEnd`] (each
    /// can break or flush where a space would not), a [`DocNode::GatedState`] (its state is
    /// the renderer's to pick), a line to replace inside a keyed or a conditional group (a
    /// copy would be a new [`GroupId`], or keep a state choice the swap made wrong — see
    /// [`Self::replace_subdoc`]), alternatives that reach the first token at different
    /// places, nothing to replace at all, or an edge deeper than the walk follows.
    pub fn glue_leading_comment_lines(&self, id: DocId, source: &str) -> Option<DocId> {
        let mut budget = LEADING_EDGE_BUDGET;
        match self.glue_walk(id, source, false, 0, &mut budget)? {
            (copy, Glue::Code | Glue::Open { .. }) if copy != id => Some(copy),
            _ => None,
        }
    }

    /// [`Self::glue_leading_comment_lines`] over `id`, entered behind a comment when
    /// `comment`: its copy (`id` itself when nothing in it changed) and where it leaves the
    /// edge. `None` declines.
    fn glue_walk(
        &self,
        id: DocId,
        source: &str,
        comment: bool,
        depth: u32,
        budget: &mut u32,
    ) -> Option<(DocId, Glue)> {
        crate::fanout_probe::walk();
        const MAX_DEPTH: u32 = 256;
        if depth > MAX_DEPTH || *budget == 0 {
            return None;
        }
        *budget -= 1;
        let node = self.nodes.borrow()[id.index()].clone();
        let open = Glue::Open { comment };
        let mut one = |inner: DocId, rebuild: &dyn Fn(DocId) -> DocNode| {
            let (copy, glue) = self.glue_walk(inner, source, comment, depth + 1, budget)?;
            Some((
                if copy == inner {
                    id
                } else {
                    self.alloc(rebuild(copy))
                },
                glue,
            ))
        };
        match node {
            DocNode::Text(t) => {
                let s = {
                    let pool = self.text_pool.borrow();
                    let s = super::types::resolve_text(&t, Some(source), &pool);
                    let s = s.trim_start_matches([' ', '\t']);
                    if s.is_empty() {
                        None
                    } else if s.starts_with("//") || (s.starts_with("/*") && s.contains('\n')) {
                        return None;
                    } else {
                        Some(s.starts_with("/*"))
                    }
                };
                Some(match s {
                    None => (id, open),
                    Some(true) => (id, Glue::Open { comment: true }),
                    Some(false) => (id, Glue::Code),
                })
            }
            DocNode::MultilineText { .. } => None,
            DocNode::Line(LineKind::Normal | LineKind::Soft) if comment => {
                Some((self.text(" "), open))
            }
            DocNode::Line(LineKind::Normal | LineKind::Soft) => Some((id, open)),
            DocNode::Line(LineKind::Hard | LineKind::Literal) => None,
            DocNode::LineSuffix(_) => Some((id, Glue::Open { comment: true })),
            DocNode::LineSuffixBoundary | DocNode::EmbedEnd { .. } | DocNode::FlushBreak => None,
            DocNode::BreakParent | DocNode::FlowProbeEnd => Some((id, open)),
            DocNode::Indent(inner) => one(inner, &DocNode::Indent),
            DocNode::Dedent(inner) => one(inner, &DocNode::Dedent),
            DocNode::AlignRoot { n, contents } => {
                one(contents, &|contents| DocNode::AlignRoot { n, contents })
            }
            DocNode::Align { n, contents } => {
                one(contents, &|contents| DocNode::Align { n, contents })
            }
            DocNode::IndentIfBreak { contents, group_id } => one(contents, &|contents| {
                DocNode::IndentIfBreak { contents, group_id }
            }),
            DocNode::WithContext { doc, context } => one(doc, &|doc| DocNode::WithContext {
                doc,
                context: context.clone(),
            }),
            DocNode::GatedState { .. } => None,
            DocNode::Group {
                contents,
                expanded_states,
                keyed,
                should_break,
            } => {
                let (copy, glue) = self.glue_walk(contents, source, comment, depth + 1, budget)?;
                if copy == contents {
                    return Some((id, glue));
                }
                if keyed || !expanded_states.is_empty() {
                    return None;
                }
                Some((
                    self.alloc(DocNode::Group {
                        contents: copy,
                        expanded_states,
                        keyed,
                        should_break,
                    }),
                    glue,
                ))
            }
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                group_id,
            } => {
                let (break_copy, break_glue) =
                    self.glue_walk(break_doc, source, comment, depth + 1, budget)?;
                let (flat_copy, flat_glue) =
                    self.glue_walk(flat_doc, source, comment, depth + 1, budget)?;
                if break_glue != flat_glue {
                    return None;
                }
                Some((
                    if break_copy == break_doc && flat_copy == flat_doc {
                        id
                    } else {
                        self.alloc(DocNode::IfBreak {
                            break_doc: break_copy,
                            flat_doc: flat_copy,
                            group_id,
                        })
                    },
                    break_glue,
                ))
            }
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let kids: DocBuf = range
                    .resolve(&self.children.borrow())
                    .iter()
                    .copied()
                    .collect();
                let mut glue = open;
                let mut copies = DocBuf::new();
                let mut changed = false;
                for kid in kids {
                    let copy = match glue {
                        Glue::Code => kid,
                        Glue::Open { comment } => {
                            let (copy, next) =
                                self.glue_walk(kid, source, comment, depth + 1, budget)?;
                            glue = next;
                            copy
                        }
                    };
                    changed |= copy != kid;
                    copies.push(copy);
                }
                if !changed {
                    return Some((id, glue));
                }
                let range = self.alloc_children(&copies);
                Some((
                    self.alloc(if matches!(node, DocNode::Concat(_)) {
                        DocNode::Concat(range)
                    } else {
                        DocNode::Fill(range)
                    }),
                    glue,
                ))
            }
        }
    }

    /// Force parent group to break — a prelude singleton (stateless, like
    /// [`Self::line`]: one shared node per document, at a fixed id).
    #[inline]
    pub fn break_parent(&self) -> DocId {
        DocId(PRELUDE_BREAK_PARENT)
    }

    /// Flush-scoped break for a deferred trailing run ([`DocNode::FlushBreak`]):
    /// force only the nearest enclosing group with a line opportunity AFTER this
    /// point — where the pending [`Self::line_suffix`] actually flushes — leaving
    /// groups that close before it free to stay flat. Emit it right after the
    /// `line_suffix` it scopes. A prelude singleton (stateless, like
    /// [`Self::break_parent`]).
    #[inline]
    pub fn flush_break(&self) -> DocId {
        DocId(PRELUDE_FLUSH_BREAK)
    }

    /// The [`DocNode::FlowProbeEnd`] sentinel, a prelude singleton — pushed only by the render loop
    /// behind a [`DocContext::flow_break_probe`]-flagged subtree, never by a printer.
    pub(super) fn flow_probe_end_node(&self) -> DocId {
        DocId(PRELUDE_FLOW_PROBE_END)
    }

    /// Open a flow probe: snapshot the output length at the probed subtree's start.
    pub(super) fn flow_probe_begin(&self, output_len: usize) {
        self.flow_probe.borrow_mut().starts.push(output_len);
    }

    /// Close the innermost flow probe, recording whether its subtree emitted a newline.
    pub(super) fn flow_probe_finish(&self, output: &str) {
        if let Some(start) = self.flow_probe.borrow_mut().starts.pop() {
            self.flow_probe_broke
                .set(output.as_bytes()[start.min(output.len())..].contains(&b'\n'));
            #[cfg(debug_assertions)]
            self.flow_probe_fresh.set(true);
        }
    }

    /// The most recently completed flow probe's answer, consumed by the paired
    /// [`DocContext::hold_line_after_broken_flow`] fill — whose command immediately follows
    /// its probe's sentinel, which is what keeps the answer fresh by construction. Debug
    /// builds assert exactly that: a read with no fresh answer (see
    /// [`Self::flow_probe_fresh`]) is a hold flag some builder set without pairing it.
    pub(super) fn flow_probe_consume(&self) -> bool {
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                self.flow_probe_fresh.get(),
                "hold_line_after_broken_flow read with no fresh flow-probe answer — \
                 the fill is not positionally paired with a flow_break_probe predecessor"
            );
            self.flow_probe_fresh.set(false);
        }
        self.flow_probe_broke.get()
    }

    /// Start a top-level render's keyed-group map empty, as prettier's `printDocToString`
    /// starts from an empty `groupModeMap`. Only the top-level render calls this; the
    /// sub-renders nested in it (fill items, line-suffix flushes) share its map. Top-level
    /// renders on one arena cannot nest (see `render_doc_iterative`), so clearing on
    /// entry is the whole lifecycle — whatever a previous render left, a panicked one
    /// included, is gone before anything reads it.
    #[inline]
    pub(super) fn clear_keyed_groups(&self) {
        self.keyed_group_modes.borrow_mut().clear();
    }

    /// Record the mode the keyed group at `group` resolved to — prettier's
    /// `groupModeMap[doc.id] = mode`, keyed on the group instance. Appended, never
    /// overwritten in place: [`Self::keyed_group_broke`] reads the newest entry for an id,
    /// so a group rendered twice reads its latest resolution.
    #[inline]
    pub(super) fn record_keyed_group(&self, group: DocId, mode: Mode) {
        self.keyed_group_modes
            .borrow_mut()
            .push((group, mode == Mode::Break));
    }

    /// Whether the group `id` names broke, in the current render. A group not yet
    /// resolved in this render reads as flat.
    #[inline]
    pub(super) fn keyed_group_broke(&self, id: GroupId) -> bool {
        let group = id.doc();
        self.keyed_group_modes
            .borrow()
            .iter()
            .rev()
            .find(|&&(entry, _)| entry == group)
            .is_some_and(|&(_, broke)| broke)
    }

    //
    // Convenience builders
    //

    /// Build a doc from items with a static string separator between them.
    ///
    /// Delegates to [`Self::join_doc`]: `text()` interns per document, so one
    /// upfront call yields the same shared separator node a per-gap `text()`
    /// would (a 0/1-item list "wastes" only the intern probe — the node
    /// almost always already exists for hot separators like `","`).
    pub fn join(&self, docs: impl IntoIterator<Item = DocId>, separator: &'static str) -> DocId {
        self.join_doc(docs, self.text(separator))
    }

    /// Build a doc from items with a Doc separator between them.
    ///
    /// Since DocId is Copy, no cloning needed for the separator.
    pub fn join_doc(&self, docs: impl IntoIterator<Item = DocId>, separator: DocId) -> DocId {
        let iter = docs.into_iter();
        let (lower, _) = iter.size_hint();
        // Shared inline buffer (N=8): the joined parts (2n-1 for n items) stay off the
        // heap for the common small list, matching the three printers' DocBuf sweep.
        // Call sites join arg/param/specifier lists (≥1 item), so the buffer is never
        // the always-empty no-op the SmallVec sweep warns about.
        let mut parts = DocBuf::with_capacity(lower.saturating_mul(2).saturating_sub(1));
        for (i, doc) in iter.enumerate() {
            if i > 0 {
                parts.push(separator); // Copy, no clone needed!
            }
            parts.push(doc);
        }
        // `concat` short-circuits empty → `empty()` and single → the element.
        self.concat(&parts)
    }

    /// Wrap a doc in parentheses.
    ///
    /// The three wrappers spell their delimiters in place rather than through a shared
    /// `wrap(open, inner, close)`: that helper stayed outlined, so the literals reached
    /// [`Self::text`] as runtime arguments and paid the prelude match at every call.
    #[inline]
    pub fn parens(&self, inner: DocId) -> DocId {
        self.concat(&[self.text("("), inner, self.text(")")])
    }

    /// Wrap a doc in square brackets.
    #[inline]
    pub fn brackets(&self, inner: DocId) -> DocId {
        self.concat(&[self.text("["), inner, self.text("]")])
    }

    /// Wrap a doc in curly braces.
    #[inline]
    pub fn braces(&self, inner: DocId) -> DocId {
        self.concat(&[self.text("{"), inner, self.text("}")])
    }

    /// Indent with leading line break.
    #[inline]
    pub fn indent_line(&self, inner: DocId) -> DocId {
        let l = self.line();
        self.indent(self.concat(&[l, inner]))
    }

    /// Indent with leading softline.
    #[inline]
    pub fn indent_softline(&self, inner: DocId) -> DocId {
        let sl = self.softline();
        self.indent(self.concat(&[sl, inner]))
    }

    /// Indent with leading hardline — the always-broken sibling of
    /// [`Self::indent_softline`].
    ///
    /// The shape every exploded container hangs its contents with: a body block's `{`,
    /// a declaration body, a broken argument or element list. Named here because it was
    /// hand-spelled at ~50 sites across the language printers, where the nesting hid
    /// which of the three `indent_*` forms a given container had actually chosen.
    #[inline]
    pub fn indent_hardline(&self, inner: DocId) -> DocId {
        let hl = self.hardline();
        self.indent(self.concat(&[hl, inner]))
    }

    /// Comma followed by line break.
    #[inline]
    pub fn comma_line(&self) -> DocId {
        self.concat(&[self.text(","), self.line()])
    }

    /// Comma followed by hardline.
    #[inline]
    pub fn comma_hardline(&self) -> DocId {
        self.concat(&[self.text(","), self.hardline()])
    }

    //
    // Tree inspection
    //

    /// Check if a doc will definitely break (contains hardline or should_break group).
    ///
    /// Memoized per `DocId`: the same subtree is re-checked many times as ancestor
    /// groups test breaking, and the result is fixed once the node exists.
    ///
    /// The body is a warm cache probe over an outlined [`Self::will_break_cold`],
    /// which is the shape the call pattern asks for: an ancestor group re-tests a
    /// subtree far more often than the subtree is first visited, so the common
    /// call reads one already-populated slot. Everything else the uncached path
    /// needs — a `nodes` borrow, a `children` borrow, and the check that extends
    /// the memo to cover `id` — is work the warm call was paying for a branch it
    /// never took, across two cache lines of the arena header, behind the five
    /// callee-saved pushes the resize edge's frame required.
    ///
    /// Worth `instructions:u` **−0.28…−0.35%** across four real corpora (per-side
    /// spread ≤0.001%) for **+5,216 B** of `.text`, on a doc-arena-free
    /// parse+bind control at +0.002%. ⚠️ A pure-CSS corpus moves **−0.090%** on
    /// this change even though the CSS printer never calls `will_break` at all:
    /// that is an incidental codegen shift, and it is the honest floor for
    /// attributing any doc-layer lever.
    #[inline]
    pub fn will_break(&self, id: DocId) -> bool {
        if let Some(&cached) = self.layout_cache.borrow().get(id.index())
            && cached != LAYOUT_UNKNOWN
        {
            return cached == LAYOUT_BREAKS_FORCED;
        }
        self.will_break_cold(id)
    }

    /// The uncached half of [`Self::will_break`]: take the node and child slices,
    /// extend the memo to cover `id`, and compute. Recursion re-enters through
    /// [`Self::subtree_layout_memo`] on the slices, never back through the probe,
    /// so the borrows are taken once per uncached root and not once per node.
    ///
    /// The fill it calls also computes the subtree's flat width, which is the
    /// point: this build-time walk visits 98.9% of the nodes the render-time fits
    /// walk later asks about, so filling both here turns that second traversal
    /// into a cache read.
    #[cold]
    #[inline(never)]
    fn will_break_cold(&self, id: DocId) -> bool {
        let nodes = self.nodes.borrow();
        let children = self.children.borrow();
        let mut cache = self.layout_cache.borrow_mut();
        if cache.len() < nodes.len() {
            cache.resize(nodes.len(), LAYOUT_UNKNOWN);
        }
        Self::subtree_layout_memo(id, &nodes, &children, cache.as_mut_slice())
            == LAYOUT_BREAKS_FORCED
    }

    /// Split into an inline probe over an outlined recursive fill: the same
    /// subtree is re-checked far more often than it is first computed, so the
    /// warm path is a load + compare at the call site instead of a full call.
    ///
    /// The probe answers **three** questions before it will call in, in this
    /// order — and the order is the whole point:
    ///
    /// 1. a warm cache slot, which is the common case and stays first;
    /// 2. a leaf `Text`, whose answer is a field of the node
    ///    ([`text_subtree_layout`]) and which is 22% of a real corpus's nodes;
    /// 3. a kind that answers from the node alone (`MultilineText`, `Line`, a
    ///    pre-broken `Group`) or forwards ONE child's value verbatim (`Indent`,
    ///    `Dedent`, `Align`, `AlignRoot`, a plain `Group`) — a third of every
    ///    first visit between them.
    ///
    /// Only the miss path runs 2 and 3, so the hot path is unchanged, and they
    /// keep those kinds out of [`Self::subtree_layout_fill`]'s `#[cold]` frame
    /// entirely. See that function's `#[cold]` note for what the frame costs.
    ///
    /// ⭐ **The peel set is a contiguous run of discriminants adjacent to
    /// `Text`'s, and that is not a coincidence — it is what makes the residual
    /// path free.** `DocText`'s four sub-tags own `DocNode` tags 0..=3
    /// (the niche), and question 3's kinds are the tags immediately above them, so
    /// two tests are `tag < 4` and one further unsigned compare; a `Concat` —
    /// 64% of what still reaches the fill — falls through both on direct,
    /// well-predicted branches. An earlier shape peeled `IndentIfBreak` and
    /// `GatedState` too, whose tags sit above a hole, and LLVM lowered the
    /// resulting set to a jump table: the fall-through then paid an INDIRECT
    /// jump on every container, and the whole lever measured **−0.173%** where
    /// this one measures **−0.377%**.
    ///
    /// ⚠️ **`inline(always)`, not `inline`, and the difference is the whole
    /// lever.** With the peel added, plain `#[inline]` was declined: LLVM
    /// outlined the probe (it appears on a board as a 4.29% symbol of its own)
    /// and every one of its ~1.6 M calls — the warm ones included — paid a real
    /// call, for `instructions:u` **+2.085%**. Forcing it back inline turns the
    /// same source into a win. The attribute alone, without the peel, rebuilds
    /// `.text` **byte-identical**, so it is buying inlining that was already
    /// happening at the smaller body — not a layout draw.
    ///
    /// Returns the packed cell (see [`LAYOUT_UNKNOWN`]) — never
    /// [`LAYOUT_UNKNOWN`] itself.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    pub(super) fn subtree_layout_memo(
        id: DocId,
        nodes: &[DocNode],
        children: &[DocId],
        cache: &mut [u32],
    ) -> u32 {
        let slot = id.index();
        let cached = cache[slot];
        if cached != LAYOUT_UNKNOWN {
            return cached;
        }
        let node = &nodes[slot];
        // A leaf `Text`'s answer is one field of the node it is already about to
        // load, so on a miss it is cheaper to give it here than to enter
        // `Self::subtree_layout_fill` — whose `#[cold]` frame spills six
        // callee-saved registers for the container arms a leaf never reaches.
        // `Text` is 22% of a real corpus's nodes and this probe is the walk's
        // own recursion, so the miss path takes it once per such node.
        if let DocNode::Text(t) = node {
            let result = text_subtree_layout(t);
            cache[slot] = result;
            return result;
        }
        // The same argument, one kind-group along: every arm below answers from
        // the node alone or forwards ONE child's value verbatim, so entering the
        // `#[cold]` frame for it buys the prologue and nothing else.
        let child = match node {
            DocNode::MultilineText { .. } => {
                cache[slot] = LAYOUT_BREAKS_FORCED;
                return LAYOUT_BREAKS_FORCED;
            }
            DocNode::Line(kind) => {
                let result = line_subtree_layout(*kind);
                cache[slot] = result;
                return result;
            }
            DocNode::Indent(inner) | DocNode::Dedent(inner) => *inner,
            DocNode::AlignRoot { contents, .. } | DocNode::Align { contents, .. } => *contents,
            DocNode::Group {
                contents,
                should_break,
                ..
            } => {
                if *should_break {
                    cache[slot] = LAYOUT_BREAKS_FORCED;
                    return LAYOUT_BREAKS_FORCED;
                }
                *contents
            }
            _ => return Self::subtree_layout_fill(id, nodes, children, cache),
        };
        let result = Self::subtree_layout_forwarded(child, nodes, children, cache);
        cache[slot] = result;
        result
    }

    /// The answer for a node reached as a forwarding node's child — this probe
    /// with its own forwarding peel removed, so the recursion is one level deep
    /// rather than a loop.
    ///
    /// That is the whole shape of the peel above: a pass-through chain is one
    /// node deep 81% of the time and two 19%, and three 0.02%, so a deeper tail
    /// goes back through [`Self::subtree_layout_fill`] — whose forwarding arms
    /// re-enter the probe — rather than costing every call site a loop and a
    /// backfill.
    ///
    /// ⚠️ **A second peel level is measured and loses, at every set size — the
    /// one-level shape is the answer, not a first instalment.** The population is
    /// real: 18.6% of chains are two deep, and 38,216 of a real corpus's 537,315
    /// calls into the fill forward one child and do nothing else. But capturing
    /// them costs a second kind test on the ~68% of calls that reach a container,
    /// and that tax exceeds the frames it saves — `instructions:u` **+0.13…
    /// +0.21%** repeating the whole peel set, **+0.07…+0.13%** narrowed to
    /// `Indent`/`Dedent` alone. Half the wide set's cost is a lowering effect:
    /// inlined into [`Self::subtree_layout_fill`] the probe is compiled in that
    /// function's `#[cold]`, size-optimized world, where the set becomes a **jump
    /// table** instead of the compare chain it forms elsewhere, so every
    /// container fall-through pays an indirect jump (nine new tables, one per
    /// inlined site). The narrow set is a compare chain, adds no table, and still
    /// loses. The deeper peel is worth −0.10…−0.14% with that attribute removed
    /// — which is not an available shape, for the reason its own note gives.
    ///
    /// ⚠️ **`inline(always)`, and the tell that it is needed is that `.text`
    /// SHRANK without it.** Under plain `#[inline]` this body is outlined and
    /// the binary loses 160 bytes — the peel's call sites collapsing into one
    /// copy — which is the same signature, in miniature, as the caller's own
    /// note above. Forced in, the build is `.text` **byte-identical** to the
    /// one-function spelling this was factored out of, so the split is
    /// presentation only and ships on that spelling's measurements.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    fn subtree_layout_forwarded(
        child: DocId,
        nodes: &[DocNode],
        children: &[DocId],
        cache: &mut [u32],
    ) -> u32 {
        let slot = child.index();
        let cached = cache[slot];
        if cached != LAYOUT_UNKNOWN {
            return cached;
        }
        if let DocNode::Text(t) = &nodes[slot] {
            let result = text_subtree_layout(t);
            cache[slot] = result;
            return result;
        }
        Self::subtree_layout_fill(child, nodes, children, cache)
    }

    /// The cold half of [`Self::subtree_layout_memo`]: compute and cache a
    /// subtree's layout facts — whether it forces a break, and if not, its
    /// break-free flat width. Runs at most once per node, and recursion goes
    /// back through the inline probe, so warm children never re-enter here.
    ///
    /// ⚠️ **Nine of the arms below are unreachable through that probe**, which
    /// peels them: `Text`, `MultilineText`, `Line`, `Indent`, `Dedent`,
    /// `Align`, `AlignRoot` and both halves of `Group`. They are kept, and must
    /// stay in sync with the peel, because the fits walk's own probe
    /// (`arena_fits`'s `flat_width_memo`) calls this function **directly** —
    /// it is a second entry point, not a second caller of the memo. Where an
    /// arm is more than a constant it is factored into one emitter the peel
    /// also calls ([`text_subtree_layout`], [`line_subtree_layout`]); the rest
    /// are one token each, and duplicating a token is cheaper than a call.
    ///
    /// ⭐ **This is the fusion of what were two identical traversals.** The
    /// forced-break question is asked at BUILD (45 printer call sites, through
    /// [`Self::will_break`]) and the flat-width question at RENDER (the
    /// `arena_fits` fast path), and both walked the whole subtree with the same
    /// per-kind dispatch — 144.9% of node-visits between them against a 75.9%
    /// union, since 98.9% of the flat widths render asks for sit on a node the
    /// build walk already visited. Answering both in one pass makes the render
    /// walk a cache read. It is only expressible because **every arm below either
    /// reports no forced break, or reports one together with an absent width** —
    /// the induction the packed cell's encoding rests on.
    ///
    /// The two questions disagree in exactly one direction, and
    /// [`LAYOUT_BREAKS_SOFT`] is that direction: a node with no flat width that
    /// forces no break. Read the arms as answering "what does this subtree do to
    /// a line?", where `BREAKS_SOFT` means "the fits walk has to look at me".
    ///
    /// ⚠️ **`#[cold]` is a claim about the call, not about the total, and it is
    /// emphatically the right one.** This function carries ~6–8% self on a
    /// real-corpus board — "once per node" is a small share of *calls* and a
    /// large share of *time* — and `#[cold]` puts it under size-optimized
    /// codegen, visibly so: it spills and immediately reloads four argument
    /// registers around the `Concat` / `Fill` arm's recursion. Dropping the
    /// attribute removes exactly that — this body and two of its callers shrink,
    /// 103 bytes between them, with no symbol outlined — and measures
    /// `instructions:u` **+0.332%** on real corpora. The callers lose far more
    /// from the un-hinted branch than the body saves, since the probe in
    /// [`Self::subtree_layout_memo`] is inlined at every call site and wants this
    /// call laid out away from its warm path. Measured against the current call
    /// mix, which is half the original calls and now 96% `Concat` — exactly the
    /// recursive arm the attribute was chosen for. Don't re-try without a new
    /// idea.
    #[cold]
    #[inline(never)]
    pub(super) fn subtree_layout_fill(
        id: DocId,
        nodes: &[DocNode],
        children: &[DocId],
        cache: &mut [u32],
    ) -> u32 {
        crate::fanout_probe::walk();
        let result: u32 = match &nodes[id.index()] {
            // Reached only from the fits walk's own probe (`arena_fits`'s
            // `flat_width_memo`); `Self::subtree_layout_memo` answers a leaf
            // `Text` without calling in at all. Both defer to the one emitter,
            // whose doc carries the verbatim-vs-newline rule and its measured
            // spellings.
            DocNode::Text(t) => text_subtree_layout(t),
            // Contains hardlines → always breaks (like the `concat([…, hardline, …])` it replaces).
            DocNode::MultilineText { .. } => LAYOUT_BREAKS_FORCED,
            DocNode::Line(kind) => line_subtree_layout(*kind),
            DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                Self::subtree_layout_memo(*inner, nodes, children, cache)
            }
            DocNode::AlignRoot { contents, .. } | DocNode::Align { contents, .. } => {
                Self::subtree_layout_memo(*contents, nodes, children, cache)
            }
            DocNode::IndentIfBreak { contents, .. } => {
                Self::subtree_layout_memo(*contents, nodes, children, cache)
            }
            // A pre-broken group breaks and has no flat width, without consulting
            // `contents` — the short-circuit both questions took separately.
            DocNode::Group {
                contents,
                should_break,
                ..
            } => {
                if *should_break {
                    LAYOUT_BREAKS_FORCED
                } else {
                    Self::subtree_layout_memo(*contents, nodes, children, cache)
                }
            }
            // The one arm where the two questions genuinely part: an `if_break`
            // never forces a break (its break arm is the *consequence* of one),
            // but its flat width is its flat arm's — so a forced break inside
            // `flat_doc` must be softened, not propagated.
            DocNode::IfBreak { flat_doc, .. } => {
                soften_forced_break(Self::subtree_layout_memo(*flat_doc, nodes, children, cache))
            }
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let mut sum: u32 = 0;
                let mut no_width = false;
                let mut forced = false;
                for &kid in range.resolve(children) {
                    let v = Self::subtree_layout_memo(kid, nodes, children, cache);
                    if v == LAYOUT_BREAKS_FORCED {
                        // Both answers are settled: a forced break in a child
                        // forces the parent and leaves it no flat width. This is
                        // the `any()` short-circuit the break walk always had.
                        forced = true;
                        break;
                    } else if v == LAYOUT_BREAKS_SOFT {
                        // The width is settled (there is none) but the break
                        // verdict is not — a later child may still force one, so
                        // the scan continues. That is what the break walk did
                        // anyway; only the width walk used to stop here.
                        no_width = true;
                    } else {
                        sum = sum.saturating_add(v);
                    }
                }
                if forced {
                    LAYOUT_BREAKS_FORCED
                } else if no_width {
                    LAYOUT_BREAKS_SOFT
                } else {
                    sum.min(LAYOUT_WIDTH_MAX)
                }
            }
            DocNode::WithContext { doc, context } => {
                let v = Self::subtree_layout_memo(*doc, nodes, children, cache);
                if v > LAYOUT_WIDTH_MAX {
                    v
                } else {
                    v.saturating_add(u32::from(context.trailing_reserve()))
                        .min(LAYOUT_WIDTH_MAX)
                }
            }
            // Neither occupies a column, but both carry the `has_line_suffix` state
            // the fits walk needs (a boundary with a suffix pending doesn't fit), and
            // a memoized width would hide them from it. SOFT means "walk it", never
            // "it breaks" — the walk's own arms charge them 0 columns.
            DocNode::LineSuffix(_) | DocNode::LineSuffixBoundary | DocNode::EmbedEnd { .. } => {
                LAYOUT_BREAKS_SOFT
            }
            DocNode::BreakParent => LAYOUT_BREAKS_FORCED,
            // Forces only the group its deferred run flushes in — decided by the
            // fits walk's pending-flush state, not by this subtree query, so a
            // containing group is NOT unconditionally broken. Carries the pending
            // state the walk needs, so it is SOFT rather than a width.
            DocNode::FlushBreak => LAYOUT_BREAKS_SOFT,
            // Render-only sentinel; zero columns, no layout effect.
            DocNode::FlowProbeEnd => 0,
            // Transparent to contents (the probe is measure-only). As a
            // conditional-group state this is never asked through the group —
            // a Group's `will_break` reads `contents` (state 0) alone, and a
            // fill's items are not states.
            DocNode::GatedState { contents, .. } => {
                Self::subtree_layout_memo(*contents, nodes, children, cache)
            }
        };
        cache[id.index()] = result;
        result
    }

    /// Check if a doc can break (contains any line elements) — Prettier's `canBreak`.
    ///
    /// The dual of [`Self::will_break`]: `will_break` asks whether a doc *must* break,
    /// this asks whether it *can*. Prettier's assignment `chooseLayout` reads it off the
    /// printed left-hand side (`canBreakLeftDoc`) to decide whether an unbreakable
    /// right-hand side — a template literal, a boolean, a number — may stay welded to the
    /// operator, or must fall through to `fluid` so the break lands after the operator
    /// instead of inside the assignment target.
    ///
    /// The root is answered inline ahead of the walk: on a real corpus nine asks in ten
    /// are a plain identifier target — a leaf `Text`, which cannot break — so the common
    /// case is one tag load behind the first borrow, and only a composite root takes the
    /// second borrow and the outlined recursion.
    #[inline]
    pub fn can_break(&self, id: DocId) -> bool {
        let nodes = self.nodes.borrow();
        if matches!(nodes[id.index()], DocNode::Text(_)) {
            return false;
        }
        let children = self.children.borrow();
        Self::can_break_inner(id, &nodes, &children)
    }

    /// The slice-threaded body of [`Self::can_break`] — `pub(super)` so the
    /// `arena_fits` walk's pending-flush veto asks it through the slices the
    /// walk already holds instead of re-borrowing per call (the threading
    /// idiom of [`Self::subtree_layout_fill`]).
    pub(super) fn can_break_inner(id: DocId, nodes: &[DocNode], children: &[DocId]) -> bool {
        Self::can_break_walk(id, nodes, children, &mut WalkMemo::new())
    }

    /// [`Self::can_break_inner`]'s walk, memoized per node ([`WalkMemo`]): the answer is a
    /// function of the node alone, and a shared subtree is reached once per path to it.
    fn can_break_walk(
        id: DocId,
        nodes: &[DocNode],
        children: &[DocId],
        memo: &mut WalkMemo<DocId, bool>,
    ) -> bool {
        crate::fanout_probe::walk();
        if let Some(known) = memo.visit(id) {
            return known;
        }
        let answer = match &nodes[id.index()] {
            DocNode::Line(_) => true,
            DocNode::FlowProbeEnd => false,
            DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                Self::can_break_walk(*inner, nodes, children, memo)
            }
            DocNode::AlignRoot { contents, .. } | DocNode::Align { contents, .. } => {
                Self::can_break_walk(*contents, nodes, children, memo)
            }
            DocNode::IndentIfBreak { contents, .. } => {
                Self::can_break_walk(*contents, nodes, children, memo)
            }
            DocNode::Group {
                contents,
                expanded_states,
                ..
            } => {
                Self::can_break_walk(*contents, nodes, children, memo)
                    || (!expanded_states.is_empty()
                        && expanded_states
                            .resolve(children)
                            .iter()
                            .any(|&kid| Self::can_break_walk(kid, nodes, children, memo)))
            }
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                ..
            } => {
                Self::can_break_walk(*break_doc, nodes, children, memo)
                    || Self::can_break_walk(*flat_doc, nodes, children, memo)
            }
            DocNode::Concat(range) | DocNode::Fill(range) => range
                .resolve(children)
                .iter()
                .any(|&kid| Self::can_break_walk(kid, nodes, children, memo)),
            DocNode::WithContext { doc, .. } => Self::can_break_walk(*doc, nodes, children, memo),
            DocNode::LineSuffix(inner) => Self::can_break_walk(*inner, nodes, children, memo),
            // Transparent to contents (the probe is measure-only).
            DocNode::GatedState { contents, .. } => {
                Self::can_break_walk(*contents, nodes, children, memo)
            }
            DocNode::MultilineText { .. } => true,
            // deliberately newline-blind, unlike `subtree_layout_fill`: canBreak asks
            // "is there a breakable `line` in here?", and a Text's embedded newline
            // (line-continuation string, verbatim slice) is content, not a break point
            DocNode::Text(_) | DocNode::LineSuffixBoundary | DocNode::EmbedEnd { .. } => false,
            DocNode::BreakParent => true,
            // No line of its own; whether a line follows is positional, which a
            // subtree query cannot see.
            DocNode::FlushBreak => false,
        };
        memo.record(id, answer)
    }

    /// Statically flatten a doc's **soft and normal** lines.
    /// Creates new nodes; old nodes remain in the arena (they're just unused).
    ///
    /// **A hard line is deliberately left alone** — prettier's `removeLinesFn`
    /// (`src/document/utilities/index.js`) gates on `!doc.hard` and says why: *"Hard lines
    /// should still output because there's too great of a chance of breaking existing
    /// assumptions otherwise."* Removing one doesn't relayout the doc, it **deletes a
    /// newline the content required**, and the content on either side fuses. A multi-line
    /// block comment in a flattened arrow signature is the case that bit us: `/* a⏎b */`
    /// came out `/* ab */`, silently gluing `a` to `b` (fixture
    /// `typescript/expressions/calls/arrow_array_return_multiline_comment`).
    ///
    /// [`DocNode::MultilineText`] is left alone for the same reason: its `\n`s *are* hard
    /// lines — the render arm emits each as a context-indented hardline — merely
    /// pre-joined into one pooled body. Flattening it is removing hard lines by another
    /// name.
    ///
    /// So this cannot promise a single line, and never could: a caller flattening a doc
    /// that contains a hard line gets a shorter doc, not a one-line one. That is prettier's
    /// contract too — `expandLastArg` flattens the signature so *breakable* params can't
    /// break, not to overrule content that must break. A caller that genuinely needs
    /// one line no matter what wants [`Self::atomize`] — a different question,
    /// and now a different name.
    pub fn remove_lines(&self, id: DocId) -> DocId {
        self.flatten_lines_impl(
            id,
            FlattenMode::RemoveLines,
            &mut WalkMemo::new(),
            FLATTEN_FRAMES,
        )
    }

    /// `root` with every occurrence of the node `target` replaced by `replacement` — a
    /// copy of the PATH from `root` down to each occurrence, every subtree off it shared.
    ///
    /// For a layout that needs a second form of a doc differing in one built piece (an
    /// enclosing conditional group's fallback state), without building the rest twice.
    /// `root` itself when `target` does not occur under it. `None` when a path to it
    /// crosses a KEYED group — a copied keyed group is a new [`GroupId`], so a reader keyed
    /// on the original would read a group that never renders — or a CONDITIONAL group,
    /// whose states a builder chose from the doc as built (a member chain's one-line state,
    /// admitted because nothing in it would break): the copy would keep a choice the
    /// swapped piece made wrong. The caller keeps its one form.
    pub fn replace_subdoc(&self, root: DocId, target: DocId, replacement: DocId) -> Option<DocId> {
        let mut memo = FxHashMap::default();
        match self.replace_subdoc_walk(root, target, replacement, &mut memo) {
            Ok(copy) => Some(copy.unwrap_or(root)),
            Err(()) => None,
        }
    }

    /// [`Self::replace_subdoc`]'s walk: `Ok(Some(copy))` when `id`'s subtree holds
    /// `target`, `Ok(None)` when it does not (the caller shares `id`), `Err(())` when a
    /// path to it crosses a keyed group. Memoized, since a doc is a DAG.
    ///
    /// A node **older** than `target` is answered without a look: the arena is append-only
    /// and a node can only name ids that already exist when it is allocated ([`Self::alloc`]
    /// is the one writer, a [`DocId`] is minted only by it or for the prelude, and
    /// [`Self::reset`] runs between documents), so every child is older than its parent and
    /// no subtree rooted below `target` can contain it. That subtree's answer is `Ok(None)`
    /// — the walk errs only on a path that holds `target` — and pruning it also skips its
    /// memo entries, which nothing else reads.
    fn replace_subdoc_walk(
        &self,
        id: DocId,
        target: DocId,
        replacement: DocId,
        memo: &mut FxHashMap<DocId, Option<DocId>>,
    ) -> Result<Option<DocId>, ()> {
        crate::fanout_probe::walk();
        if id == target {
            return Ok(Some(replacement));
        }
        if id.0 < target.0 {
            return Ok(None);
        }
        if let Some(&known) = memo.get(&id) {
            return Ok(known);
        }
        let node = self.nodes.borrow()[id.index()].clone();
        let mut walk = |child: DocId| -> Result<(DocId, bool), ()> {
            debug_assert!(child.0 < id.0, "a doc node names only older nodes");
            Ok(
                match self.replace_subdoc_walk(child, target, replacement, memo)? {
                    Some(copy) => (copy, true),
                    None => (child, false),
                },
            )
        };
        let copy = match node {
            DocNode::Text(_)
            | DocNode::MultilineText { .. }
            | DocNode::Line(_)
            | DocNode::LineSuffixBoundary
            | DocNode::EmbedEnd { .. }
            | DocNode::BreakParent
            | DocNode::FlushBreak
            | DocNode::FlowProbeEnd => None,
            DocNode::Indent(inner) => match walk(inner)? {
                (copy, true) => Some(DocNode::Indent(copy)),
                _ => None,
            },
            DocNode::Dedent(inner) => match walk(inner)? {
                (copy, true) => Some(DocNode::Dedent(copy)),
                _ => None,
            },
            DocNode::LineSuffix(inner) => match walk(inner)? {
                (copy, true) => Some(DocNode::LineSuffix(copy)),
                _ => None,
            },
            DocNode::AlignRoot { n, contents } => match walk(contents)? {
                (copy, true) => Some(DocNode::AlignRoot { n, contents: copy }),
                _ => None,
            },
            DocNode::Align { n, contents } => match walk(contents)? {
                (copy, true) => Some(DocNode::Align { n, contents: copy }),
                _ => None,
            },
            DocNode::WithContext { doc, context } => match walk(doc)? {
                (copy, true) => Some(DocNode::WithContext { doc: copy, context }),
                _ => None,
            },
            DocNode::IndentIfBreak { contents, group_id } => match walk(contents)? {
                (copy, true) => Some(DocNode::IndentIfBreak {
                    contents: copy,
                    group_id,
                }),
                _ => None,
            },
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                group_id,
            } => {
                let (break_copy, break_changed) = walk(break_doc)?;
                let (flat_copy, flat_changed) = walk(flat_doc)?;
                (break_changed || flat_changed).then_some(DocNode::IfBreak {
                    break_doc: break_copy,
                    flat_doc: flat_copy,
                    group_id,
                })
            }
            DocNode::GatedState {
                probe,
                contents,
                admit,
            } => {
                let (probe_copy, probe_changed) = walk(probe)?;
                let (contents_copy, contents_changed) = walk(contents)?;
                (probe_changed || contents_changed).then_some(DocNode::GatedState {
                    probe: probe_copy,
                    contents: contents_copy,
                    admit,
                })
            }
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let kids: DocBuf = range
                    .resolve(&self.children.borrow())
                    .iter()
                    .copied()
                    .collect();
                let mut changed = false;
                let mut copies = DocBuf::new();
                for kid in kids {
                    let (copy, kid_changed) = walk(kid)?;
                    changed |= kid_changed;
                    copies.push(copy);
                }
                changed.then(|| {
                    let range = self.alloc_children(&copies);
                    if matches!(node, DocNode::Concat(_)) {
                        DocNode::Concat(range)
                    } else {
                        DocNode::Fill(range)
                    }
                })
            }
            DocNode::Group {
                contents,
                expanded_states,
                keyed,
                should_break,
            } => {
                let states: DocBuf = expanded_states
                    .resolve(&self.children.borrow())
                    .iter()
                    .copied()
                    .collect();
                let (contents_copy, mut changed) = walk(contents)?;
                let mut state_copies = DocBuf::new();
                for state in states {
                    let (copy, state_changed) = walk(state)?;
                    changed |= state_changed;
                    state_copies.push(copy);
                }
                // A conditional group's states were chosen and ordered at build time from
                // the doc it holds (a member chain's one-line and expanded forms, picked by
                // whether the one-line form will break): a copy with a piece swapped keeps
                // that choice, so its render can admit a state the swapped piece made
                // wrong. Declined like a keyed group.
                if changed && (keyed || !state_copies.is_empty()) {
                    return Err(());
                }
                changed.then(|| DocNode::Group {
                    contents: contents_copy,
                    expanded_states: self.alloc_children(&state_copies),
                    keyed,
                    should_break,
                })
            }
        };
        let result = copy.map(|node| self.alloc(node));
        memo.insert(id, result);
        Ok(result)
    }

    /// Force a doc onto **one line at any width** — every line flattened, hard ones
    /// included.
    ///
    /// Not prettier's `removeLines` (that is [`Self::remove_lines`], which keeps hard
    /// lines and cannot promise one line). Prettier gets here by re-rendering the doc at
    /// `printWidth: Infinity` and substituting the resulting string
    /// (`template-literal.js`); this achieves the same as a doc transform, which is why it
    /// is named for that contract rather than for the line-flattening mechanism.
    ///
    /// **Emulating a re-render, not a stronger `removeLines`** — the difference is
    /// load-bearing at every node where "what would infinite width print?" and "what does
    /// flattening this node yield?" disagree. A `conditional_group` is such a node: at
    /// infinite width its least-expanded state always fits and wins, so this **collapses
    /// it to that state**. Prettier's `removeLines` instead keeps the states (its `mapDoc`
    /// re-derives `contents = expandedStates[0]`), which [`Self::remove_lines`] mirrors —
    /// tsv's `contents` *is* `state[0]`, so recursing both is the same thing. Keeping the
    /// states here was a bug: render found none fitting at the real width, fell back to
    /// the most-expanded one, and printed its already-flattened separators as literal
    /// spaces (`xs.map( (i) => fn(i) )`).
    ///
    /// The invariant that falls out, and that the tests assert: **the result renders
    /// identically at every width.**
    ///
    /// **Only sound where the content provably has no required newline.** Deleting a hard
    /// line does not relayout anything — it deletes a newline the content demanded, fusing
    /// whatever sat on either side (`/* a⏎b */` → `/* ab */`, gluing `a` to `b`). Its one
    /// caller, the template-interpolation atomizer, first routes any interpolation whose doc
    /// will break, or whose source spans lines (an interior `//` or multi-line comment among
    /// them), down a *different* branch, so nothing that must break can reach here. A
    /// single-line block comment can reach it, and carries no newline.
    ///
    /// Folding the two into one function under prettier's name is how a multi-line comment
    /// in a flattened arrow signature gets its newline deleted. Two questions, two names.
    pub fn atomize(&self, id: DocId) -> DocId {
        self.flatten_lines_impl(
            id,
            FlattenMode::Atomize,
            &mut WalkMemo::new(),
            FLATTEN_FRAMES,
        )
    }

    /// [`Self::remove_lines`] and [`Self::atomize`]'s walk, memoized per node within one
    /// call: a node's flattened copy is a function of the node and the call's `mode`, and
    /// render reads a doc by its nodes, never their ids' positions, so a subtree shared
    /// in the input stays shared in the copy (a copied group is never keyed).
    ///
    /// Recursive while `frames` doc levels of it are left to spend ([`FLATTEN_FRAMES`]
    /// from the entry points); below them the subtree is walked by
    /// [`Self::flatten_lines_loop`], so a doc's depth costs a bounded stack.
    fn flatten_lines_impl(
        &self,
        id: DocId,
        mode: FlattenMode,
        memo: &mut WalkMemo<DocId, DocId>,
        frames: u32,
    ) -> DocId {
        if frames == 0 {
            return self.flatten_lines_loop(id, mode, memo);
        }
        crate::fanout_probe::walk();
        if let Some(known) = memo.visit(id) {
            return known;
        }
        let info = self.flatten_info(id, mode);
        #[cfg(feature = "comment_check")]
        let retags = info.retags(mode);
        let new_id = self.flatten_build(id, info, mode, &mut |kid| {
            self.flatten_lines_impl(kid, mode, memo, frames - 1)
        });
        #[cfg(feature = "comment_check")]
        self.flatten_retag(id, new_id, retags);
        memo.record(id, new_id)
    }

    /// [`Self::flatten_lines_impl`]'s walk of the subtree under `root` without recursing:
    /// a post-order loop over an explicit stack, which visits, flattens and allocates in
    /// the recursion's order — each node's children in order, each copy after its
    /// children's — so its copy, its node ids and the memo's entries are the recursion's.
    #[cold]
    #[inline(never)]
    fn flatten_lines_loop(
        &self,
        root: DocId,
        mode: FlattenMode,
        memo: &mut WalkMemo<DocId, DocId>,
    ) -> DocId {
        /// A node to visit, or a visited node to copy once its children are: its info,
        /// and where its children's copies start on the result stack.
        enum Step {
            Enter(DocId),
            Exit(DocId, FlattenInfo, usize),
        }
        let mut steps = vec![Step::Enter(root)];
        // the copies of the visited nodes not yet taken by their parent, in visit order
        let mut copies: Vec<DocId> = Vec::new();
        let mut kids = DocBuf::new();
        while let Some(step) = steps.pop() {
            match step {
                Step::Enter(id) => {
                    crate::fanout_probe::walk();
                    if let Some(known) = memo.visit(id) {
                        copies.push(known);
                        continue;
                    }
                    let info = self.flatten_info(id, mode);
                    kids.clear();
                    self.flatten_children(&info, mode, &mut kids);
                    steps.push(Step::Exit(id, info, copies.len()));
                    steps.extend(kids.iter().rev().map(|&kid| Step::Enter(kid)));
                }
                Step::Exit(id, info, first) => {
                    #[cfg(feature = "comment_check")]
                    let retags = info.retags(mode);
                    let mut next = first;
                    let new_id = self.flatten_build(id, info, mode, &mut |_| {
                        let copy = copies[next];
                        next += 1;
                        copy
                    });
                    debug_assert_eq!(
                        next,
                        copies.len(),
                        "the copy took every child the loop flattened"
                    );
                    copies.truncate(first);
                    #[cfg(feature = "comment_check")]
                    self.flatten_retag(id, new_id, retags);
                    copies.push(memo.record(id, new_id));
                }
            }
        }
        debug_assert_eq!(copies.len(), 1, "the root's copy is the loop's one result");
        copies[0]
    }

    /// What [`Self::flatten_build`] reads of node `id`, taken while the arena is borrowed —
    /// released before the build allocates, since `alloc()` needs `borrow_mut()`.
    #[inline]
    fn flatten_info(&self, id: DocId, mode: FlattenMode) -> FlattenInfo {
        let nodes = self.nodes.borrow();
        match &nodes[id.index()] {
            DocNode::Text(_)
            | DocNode::LineSuffixBoundary
            | DocNode::EmbedEnd { .. }
            | DocNode::FlowProbeEnd => FlattenInfo::Keep,
            // `MultilineText`'s `\n`s are hard lines pre-joined into one body, so it
            // follows `mode` for the same reason a `Line(Hard)` does — see `remove_lines`.
            DocNode::MultilineText { span, .. } => match mode {
                FlattenMode::RemoveLines => FlattenInfo::Keep,
                FlattenMode::Atomize => {
                    let pool = self.text_pool.borrow();
                    FlattenInfo::FlattenedMultilineText(span.slice(&pool).replace('\n', ""))
                }
            },
            DocNode::Line(kind) => FlattenInfo::Line(*kind),
            DocNode::Indent(inner) => FlattenInfo::Indent(*inner),
            DocNode::Dedent(inner) => FlattenInfo::Dedent(*inner),
            DocNode::AlignRoot { n, contents } => FlattenInfo::AlignRoot(*n, *contents),
            DocNode::Align { n, contents } => FlattenInfo::Align(*n, *contents),
            // A copied group is a new instance, so it is never keyed: at every caller a
            // keyed group's readers sit in the same flattened subtree as the group (a
            // chain's heads and its body indent / callee `)`, a head group and its
            // dangle), and flatten away below — to the flat arm / the bare contents —
            // so nothing is left to key on the copy.
            DocNode::Group {
                contents,
                expanded_states,
                should_break,
                ..
            } => FlattenInfo::Group {
                contents: *contents,
                expanded_states: *expanded_states,
                should_break: *should_break,
            },
            DocNode::IfBreak { flat_doc, .. } => FlattenInfo::IfBreakFlat(*flat_doc),
            DocNode::IndentIfBreak { contents, .. } => {
                FlattenInfo::IndentIfBreakContents(*contents)
            }
            DocNode::Concat(range) => {
                let children = self.children.borrow();
                FlattenInfo::Concat(DocBuf::from_slice(range.resolve(&children)))
            }
            DocNode::Fill(range) => {
                let children = self.children.borrow();
                FlattenInfo::Fill(DocBuf::from_slice(range.resolve(&children)))
            }
            DocNode::WithContext { doc, context } => {
                FlattenInfo::WithContext(*doc, context.clone())
            }
            DocNode::LineSuffix(inner) => FlattenInfo::LineSuffix(*inner),
            // Both are pure layout-forcing markers with no content: flattening
            // drops them the same way (`FlattenInfo::BreakParent` → `empty()`).
            DocNode::BreakParent | DocNode::FlushBreak => FlattenInfo::BreakParent,
            DocNode::GatedState {
                probe,
                contents,
                admit,
            } => FlattenInfo::GatedState(*probe, *contents, *admit),
        }
    }

    /// The children [`Self::flatten_build`] asks the copies of for `info`, in the order it
    /// asks — what [`Self::flatten_lines_loop`] flattens ahead of the build.
    fn flatten_children(&self, info: &FlattenInfo, mode: FlattenMode, out: &mut DocBuf) {
        match info {
            FlattenInfo::Keep
            | FlattenInfo::FlattenedMultilineText(_)
            | FlattenInfo::Line(_)
            | FlattenInfo::BreakParent => {}
            FlattenInfo::Indent(inner)
            | FlattenInfo::Dedent(inner)
            | FlattenInfo::AlignRoot(_, inner)
            | FlattenInfo::Align(_, inner)
            | FlattenInfo::IfBreakFlat(inner)
            | FlattenInfo::IndentIfBreakContents(inner)
            | FlattenInfo::WithContext(inner, _)
            | FlattenInfo::LineSuffix(inner) => out.push(*inner),
            FlattenInfo::Group {
                contents,
                expanded_states,
                should_break,
            } => {
                out.push(*contents);
                if mode == FlattenMode::RemoveLines && !should_break {
                    let children = self.children.borrow();
                    out.extend_from_slice(expanded_states.resolve(&children));
                }
            }
            FlattenInfo::Concat(kids) | FlattenInfo::Fill(kids) => out.extend_from_slice(kids),
            FlattenInfo::GatedState(probe, contents, _) => {
                out.push(*probe);
                out.push(*contents);
            }
        }
    }

    /// The flattened copy of node `id`, read as `info`, its children's copies taken from
    /// `flatten` in [`Self::flatten_children`]'s order — by recursion
    /// ([`Self::flatten_lines_impl`]) or from the loop's result stack
    /// ([`Self::flatten_lines_loop`]). Out of line over a `dyn` closure, so the two walks
    /// share one body.
    #[inline(never)]
    fn flatten_build(
        &self,
        id: DocId,
        info: FlattenInfo,
        mode: FlattenMode,
        flatten: &mut dyn FnMut(DocId) -> DocId,
    ) -> DocId {
        match info {
            FlattenInfo::Keep => id,
            FlattenInfo::FlattenedMultilineText(flat) => self.text_pooled(&flat),
            FlattenInfo::Line(kind) => match kind {
                LineKind::Normal => self.text(" "),
                LineKind::Soft => self.empty(),
                // Prettier's `!doc.hard` gate: a hard line passes through untouched, because
                // removing one deletes a required newline rather than relayouting anything.
                // Only `atomize`, whose content provably has no required newline,
                // drops them.
                LineKind::Hard | LineKind::Literal => match mode {
                    FlattenMode::RemoveLines => id,
                    FlattenMode::Atomize => self.empty(),
                },
            },
            FlattenInfo::Indent(inner) => {
                let new_inner = flatten(inner);
                self.indent(new_inner)
            }
            FlattenInfo::Dedent(inner) => {
                let new_inner = flatten(inner);
                self.dedent(new_inner)
            }
            FlattenInfo::AlignRoot(n, contents) => {
                let new_contents = flatten(contents);
                self.align_root(n, new_contents)
            }
            FlattenInfo::Align(n, contents) => {
                let new_contents = flatten(contents);
                self.align(n, new_contents)
            }
            FlattenInfo::Group {
                contents,
                expanded_states,
                should_break,
            } => {
                let flat_contents = flatten(contents);
                if mode == FlattenMode::Atomize {
                    // Atomize: emulate prettier's re-render at `printWidth: Infinity`, where a
                    // conditional group's *least*-expanded state always fits and is chosen. So
                    // the expanded states are dead here — drop them.
                    //
                    // Recursing into them instead (as the `remove_lines` arm below does) is a
                    // bug: the states keep their `line` docs, which this transform has just
                    // flattened to spaces / nothing. Render then finds no state fits at the
                    // real width, falls back to the most-expanded one, and emits its separators
                    // as literal spaces — `xs.map( (i) => fn(i) )` — or, when that state's
                    // separator was a `softline`, deletes a required one: `(i) =>fn(i)`.
                    return self.alloc(DocNode::Group {
                        contents: flat_contents,
                        expanded_states: ChildRange::EMPTY,
                        keyed: false,
                        should_break,
                    });
                }
                if should_break {
                    self.alloc(DocNode::Group {
                        contents: flat_contents,
                        expanded_states, // Keep as-is
                        keyed: false,
                        should_break,
                    })
                } else {
                    let flat_states = if expanded_states.is_empty() {
                        ChildRange::EMPTY
                    } else {
                        let kids = {
                            let children = self.children.borrow();
                            DocBuf::from_slice(expanded_states.resolve(&children))
                        };
                        let new_kids: DocBuf = kids.into_iter().map(&mut *flatten).collect();
                        self.alloc_children(&new_kids)
                    };
                    self.alloc(DocNode::Group {
                        contents: flat_contents,
                        expanded_states: flat_states,
                        keyed: false,
                        should_break,
                    })
                }
            }
            FlattenInfo::IfBreakFlat(flat_doc) => flatten(flat_doc),
            FlattenInfo::IndentIfBreakContents(contents) => flatten(contents),
            FlattenInfo::Concat(kids) => {
                // A flattened `softline` is `empty()`; a concat carries no empty child,
                // since every later walk would visit it for nothing.
                let empty = self.empty();
                self.concat_iter(
                    kids.into_iter()
                        .map(&mut *flatten)
                        .filter(|&kid| kid != empty),
                )
            }
            FlattenInfo::Fill(kids) => {
                // Fill becomes regular concat when flattened — and, as a concat, carries
                // no empty child either (a flattened softline separator). The parity a
                // `Fill` keeps between its items and separators is gone with the fill.
                let empty = self.empty();
                self.concat_iter(
                    kids.into_iter()
                        .map(&mut *flatten)
                        .filter(|&kid| kid != empty),
                )
            }
            FlattenInfo::WithContext(doc, context) => {
                let new_doc = flatten(doc);
                self.with_context(new_doc, context)
            }
            FlattenInfo::LineSuffix(inner) => {
                let new_inner = flatten(inner);
                self.line_suffix(new_inner)
            }
            FlattenInfo::BreakParent => self.empty(),
            // Rebuild both halves, keeping the gate: under `remove_lines` (which
            // keeps conditional-group states) the state must stay gated; under
            // `atomize` the node is unreachable through a group (states dropped)
            // and the rebuild is inert.
            FlattenInfo::GatedState(probe, contents, admit) => {
                let new_probe = flatten(probe);
                let new_contents = flatten(contents);
                self.gated_state_admitting(new_probe, new_contents, admit)
            }
        }
    }

    /// Rebuilding the tree strands a comment doc's ledger tag on the discarded original
    /// (a re-allocated node gets a fresh `DocId`), so the renderer never records the emit
    /// and the — verbatim-printed — comment reads as DROPPED. Carry the tag onto each
    /// rebuilt node. Every node of the walk routes through here, so a tagged comment
    /// anywhere in the subtree is covered. `comment_check`-only — production output is
    /// byte-identical.
    #[cfg(feature = "comment_check")]
    fn flatten_retag(&self, id: DocId, new_id: DocId, retags: bool) {
        if retags && new_id != id {
            self.retag_comment_doc(new_id, id);
        }
    }

    //
    // Node access (for rendering)
    //

    /// Get a reference to the node at the given DocId.
    ///
    /// For tight loops during rendering, prefer borrowing the full nodes vec
    /// once with `borrow_nodes()`.
    #[inline]
    pub fn get(&self, id: DocId) -> std::cell::Ref<'_, DocNode> {
        std::cell::Ref::map(self.nodes.borrow(), |nodes| &nodes[id.index()])
    }

    /// The span of a doc that is exactly one [`DocText::SourceSpan`] text, else `None`.
    ///
    /// Through the **preserve-whitespace** render entry
    /// (`arena_print_doc_with_indent_resolved_preserve_whitespace_into`) such a doc
    /// renders as `span.extract(source)` and nothing else, at any column, indent and
    /// width: a lone text holds no line for the renderer to decide, and the text arm
    /// appends the resolved slice whatever the width slot says. A printer that writes
    /// each piece through that entry to a buffer of its own can therefore append the
    /// slice itself and skip the render.
    ///
    /// ⚠️ Not so through an entry that trims the rendered piece's last line
    /// (`arena_print_doc_with_indent_resolved_into`): a slice may END in a space or tab
    /// of its own (a CSS escape's payload, `50px\ `), which that trim strips, so a
    /// caller rendering there must not substitute the slice.
    ///
    /// ⚠️ A slice written that way never reaches the renderer's two audit seams (the
    /// `swallow_check` text hook and the `comment_check` tag record). That is sound only
    /// for a printer whose docs carry neither a line-comment mark nor a comment tag — the
    /// CSS printer, which records its comments where it writes them.
    #[inline]
    pub fn as_source_span_text(&self, id: DocId) -> Option<Span> {
        match &self.nodes.borrow()[id.index()] {
            DocNode::Text(DocText::SourceSpan(span, _)) => Some(*span),
            _ => None,
        }
    }

    /// If this DocId points to a Group node, return its contents (unwrapping the group).
    /// Otherwise return the DocId unchanged.
    #[inline]
    pub fn unwrap_group(&self, id: DocId) -> DocId {
        let nodes = self.nodes.borrow();
        match &nodes[id.index()] {
            DocNode::Group { contents, .. } => *contents,
            _ => id,
        }
    }

    /// Borrow the full nodes vec for rendering.
    #[inline]
    pub fn borrow_nodes(&self) -> std::cell::Ref<'_, Vec<DocNode>> {
        self.nodes.borrow()
    }

    /// Borrow the full children vec for rendering.
    #[inline]
    pub fn borrow_children(&self) -> std::cell::Ref<'_, Vec<DocId>> {
        self.children.borrow()
    }

    /// Borrow the arena text pool for rendering — the backing store the
    /// [`DocText::Pooled`] / [`DocNode::MultilineText`] spans index into.
    /// Hoisted once per render alongside `borrow_nodes`; the fits walk never
    /// needs it (pooled widths are always precomputed on the node).
    #[inline]
    pub(super) fn borrow_text_pool(&self) -> std::cell::Ref<'_, String> {
        self.text_pool.borrow()
    }

    /// Take the parked render-output scratch (logically empty; warm capacity
    /// when previously parked). Pair with [`Self::park_render_scratch`]; a
    /// nested taker gets the `Cell`'s empty default and warms its own buffer,
    /// so overlapping renders stay correct.
    #[inline]
    pub fn take_render_scratch(&self) -> String {
        self.render_scratch.take()
    }

    /// Park the render-output scratch back for the next render, retaining its
    /// capacity (cleared here, so it is always logically empty while parked).
    #[inline]
    pub fn park_render_scratch(&self, mut scratch: String) {
        scratch.clear();
        self.render_scratch.set(scratch);
    }

    /// Acquire a cleared [`DocBuf`] assembly buffer from the free-list (or a
    /// fresh empty one). Prefer the RAII [`Self::pooled_docbuf`]; pair a raw
    /// acquire with [`Self::release_docbuf`]. Recursion-safe: nested builders
    /// each pop a distinct buffer (or `new`), so overlapping assembly stays
    /// correct, and the pool self-sizes to the max concurrent-live buffers.
    #[inline]
    pub fn acquire_docbuf(&self) -> DocBuf {
        self.docbuf_pool.borrow_mut().pop().unwrap_or_default()
    }

    /// Return a [`DocBuf`] to the free-list, cleared (capacity retained), for a
    /// later builder to reuse. Only affects allocation, never output.
    ///
    /// Only *spilled* buffers are worth keeping: a never-spilled `DocBuf` has no
    /// heap capacity to retain and costs nothing to construct fresh
    /// (`acquire_docbuf`'s `unwrap_or_default`), so pooling it would only bury
    /// the capacity-bearing buffers deeper in the LIFO — a big-need builder
    /// popping a virgin buffer while a spilled one sits below it re-pays the
    /// spill malloc. Dropping virgins keeps every pooled entry capacity-bearing,
    /// so the pop always hands back real capacity when any is free.
    #[inline]
    pub fn release_docbuf(&self, mut buf: DocBuf) {
        if buf.spilled() {
            buf.clear();
            self.docbuf_pool.borrow_mut().push(buf);
        }
    }

    /// RAII form of [`Self::acquire_docbuf`]: a [`PooledDocBuf`] that derefs to
    /// the buffer for assembly and, on drop, returns it to the pool (called
    /// after the builder's `concat`/`fill` has copied the parts into the arena).
    #[inline]
    pub fn pooled_docbuf(&self) -> PooledDocBuf<'_> {
        PooledDocBuf {
            buf: self.acquire_docbuf(),
            arena: self,
        }
    }

    /// The parked member-chain argument share (see the field doc). Returned as the
    /// `RefCell` itself — the consumer's share scope spans many interleaved
    /// arena calls, so it borrows point-wise per lookup/insert/clear rather
    /// than holding a `RefMut` open. The consumer owns the clear-at-scope-
    /// entry/exit protocol; this accessor deliberately does NOT clear.
    #[inline]
    pub fn chain_share_store(&self) -> &RefCell<ChainShareStore> {
        &self.chain_share_store
    }

    /// Borrow the pooled line-offset scratch (cleared here) — a multi-line
    /// block-comment builder fills it with each body line's `(start, end)`
    /// byte range from one `split('\n')` pass and drops the borrow before the
    /// next comment builds. Held only within one builder call; nothing
    /// downstream of the fill re-borrows it.
    #[inline]
    pub fn borrow_line_spans_scratch(&self) -> std::cell::RefMut<'_, Vec<(u32, u32)>> {
        let mut scratch = self.line_spans_scratch.borrow_mut();
        scratch.clear();
        scratch
    }

    /// Borrow the parked top-level render command stack (cleared here). Held for
    /// the duration of one top-level render; nested renders take
    /// [`Self::take_sub_render_stack`] and never take this borrow.
    #[inline]
    pub(super) fn borrow_top_render_stack(&self) -> std::cell::RefMut<'_, CmdStack> {
        let mut stack = self.top_render_stack.borrow_mut();
        stack.clear();
        stack
    }

    /// Take a command stack for a sub-render, warm if the park holds one.
    ///
    /// The companion of [`Self::borrow_top_render_stack`] for the renders
    /// that nest: a fill segment and a line-suffix flush both render inside an
    /// enclosing render, which is already holding the parked top-level stack, so
    /// they cannot borrow it — and sub-renders outnumber top-level renders four
    /// to one on a real corpus (zzz: 6,418 against 1,463), so growing a fresh
    /// `Vec` from nothing at each of them is a measurable tax: 0.84 points of a
    /// fuz_ui pass, and 0.24 of a pure-CSS one.
    ///
    /// A [`Cell`] rather than a [`RefCell`] free list, and the nesting falls out
    /// of `take` on its own: the taker leaves an empty `Vec` behind, so a render
    /// nested inside this one gets that empty stack and grows its own. The
    /// innermost render parks first and the outermost parks last, so the slot
    /// ends up holding the largest of them — which is the one worth keeping.
    /// A stack lost to an unwind is simply not parked.
    #[inline]
    pub(super) fn take_sub_render_stack(&self) -> CmdStack {
        let mut stack = self.sub_render_stack.take();
        stack.clear();
        stack
    }

    /// Park a sub-render's command stack — see [`Self::take_sub_render_stack`].
    #[inline]
    pub(super) fn return_sub_render_stack(&self, stack: CmdStack) {
        self.sub_render_stack.set(stack);
    }

    /// Borrow the pooled top-level line-suffix buffer (cleared here) — the
    /// companion of [`Self::borrow_top_render_stack`].
    #[inline]
    pub(super) fn borrow_line_suffix_scratch(&self) -> std::cell::RefMut<'_, LineSuffixBuf> {
        let mut scratch = self.line_suffix_scratch.borrow_mut();
        scratch.clear();
        scratch
    }

    /// Take the parked line-break-table scratch (logically empty; warm
    /// capacity when previously parked). Pair with
    /// [`Self::park_line_breaks_scratch`]; a nested taker gets the `Cell`'s
    /// empty default and simply warms its own table.
    #[inline]
    pub fn take_line_breaks_scratch(&self) -> Vec<u32> {
        self.line_breaks_scratch.take()
    }

    /// Park the line-break table back for the next format, retaining its
    /// capacity (cleared here, so it is always logically empty while parked).
    #[inline]
    pub fn park_line_breaks_scratch(&self, mut breaks: Vec<u32>) {
        breaks.clear();
        self.line_breaks_scratch.set(breaks);
    }

    /// Mutably borrow the subtree-layout cache for the `arena_fits` fast-path.
    #[inline]
    pub(super) fn borrow_layout_cache(&self) -> std::cell::RefMut<'_, Vec<u32>> {
        self.layout_cache.borrow_mut()
    }

    /// Estimate output buffer capacity (bytes) for a rendered string.
    ///
    /// Consumer shape: this sizes the reservation for the **per-render-call**
    /// output the `arena_print_doc*` entry points write — usually the
    /// arena-parked scratch the `*_into` forms render into (reserved at each
    /// call; a no-op once the warm scratch has grown past it) rather than a
    /// freshly allocated `String`. Render granularity is per *piece*:
    /// standalone TS renders the whole program doc in one call (plus one call
    /// per template expression when Svelte-embedded), CSS renders per
    /// selector/declaration/value, Svelte per root node and per
    /// `<script>`/`<style>` block. The file-level buffer is sized from
    /// **source length** (`Printer::with_context`'s `buffer_capacity`), not
    /// this estimate. Mid-render-sequence, `nodes.len()` is the *cumulative*
    /// count at that point — an over-estimate for later pieces, absorbed by
    /// the pooled scratch as retained capacity (bounded by the largest
    /// reservation; whether the per-piece reserve still earns its keep on the
    /// warm path is an open calibration question — dropping it must be gated
    /// on the WASM memory probe).
    ///
    /// Pre-interning calibration: rendered output measured **~1.9 bytes per doc
    /// node** (aggregate 1.888×nodes = 1.00×source), so `nodes.len() * 2` reserved
    /// with a few-percent headroom — big files (which dominate the `realloc`
    /// memcpy cost) carry the aggregate ratio and so fit in one reservation, while
    /// only small, high-ratio files pay an (amortized, cheap) realloc. This
    /// avoids the geometric `realloc`+memcpy chain a small default capacity pays
    /// (~2–3 grows per format); output writes are ~8% of the format profile, so
    /// eliminating those memcpys is a native + WASM wall lever.
    ///
    /// The prior `nodes.len() / 4` was calibrated to the old 4-nodes/byte pre-size
    /// (then `nodes/4 ≈ source ≈ output`) and under-provisioned the real output
    /// ~3.8× → every format reallocated 2–3 times. The multiplier tracks the
    /// node-interning ratchet: each interning pass dedupes nodes with output
    /// unchanged, raising output/node, so the multiplier moves in lockstep or
    /// the reallocs it exists to prevent creep back in. It went 2 → 4 with
    /// static-text node interning (per-file output/node p50 ~2.1 → ~3.1) and
    /// 4 → 5 with the singleton Line/boundary interning (p50 ~3.4–3.7,
    /// aggregate ~3.9–4.0×nodes — ×4 would have run at ~1.0× aggregate
    /// clearance, i.e. zero headroom; ×5 restores the ~1.3× the ×4 tuning had,
    /// `arena_stats` calibration).
    ///
    /// Floor: 256 bytes (tiny inputs). Ceiling: 1 GiB — a pure sanity backstop
    /// that no real format approaches (the estimate tracks the actual node count),
    /// raised from the old 1 MiB which capped any file whose output exceeded 1 MB
    /// and re-introduced reallocs on large files.
    #[inline]
    pub fn estimated_output_capacity(&self) -> usize {
        (self.nodes.borrow().len() * 5).clamp(256, 1 << 30)
    }
}

/// RAII guard wrapping a pooled [`DocBuf`] (see [`DocArena::pooled_docbuf`]).
/// Derefs to the buffer for assembly; on drop, returns it to the arena's
/// free-list (cleared, capacity retained). `#![forbid(unsafe_code)]`-clean: the
/// drop moves the buffer out via `mem::take` (`DocBuf: Default`), leaving a
/// stack-only empty `SmallVec` to drop as a no-op.
pub struct PooledDocBuf<'a> {
    buf: DocBuf,
    arena: &'a DocArena,
}

impl std::ops::Deref for PooledDocBuf<'_> {
    type Target = DocBuf;
    #[inline]
    fn deref(&self) -> &DocBuf {
        &self.buf
    }
}

impl std::ops::DerefMut for PooledDocBuf<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut DocBuf {
        &mut self.buf
    }
}

impl Drop for PooledDocBuf<'_> {
    #[inline]
    fn drop(&mut self) {
        self.arena.release_docbuf(std::mem::take(&mut self.buf));
    }
}

impl Default for DocArena {
    fn default() -> Self {
        Self::new()
    }
}

/// Streaming builder for pooled text — see [`DocArena::pool_writer`].
///
/// Assembles a dynamic string piecewise in an arena-parked scratch buffer,
/// replacing the `let s = format!(…); d.text_pooled(&s)` pattern (same copy
/// count — assembly + one pool copy — minus the transient `String`
/// alloc/dealloc pair per call). Implements [`std::fmt::Write`] (never errors) so
/// `write!(w, …)` works for formatted pieces; plain pieces use the infallible
/// [`Self::push_str`] / [`Self::push`].
pub struct PoolTextWriter<'a> {
    arena: &'a DocArena,
    scratch: String,
}

impl PoolTextWriter<'_> {
    /// Append a string piece.
    #[inline]
    pub fn push_str(&mut self, s: &str) {
        self.scratch.push_str(s);
    }

    /// Append a single char.
    #[inline]
    pub fn push(&mut self, c: char) {
        self.scratch.push(c);
    }

    /// Reserve for at least `additional` more bytes (optional — the scratch
    /// capacity is retained across uses, so steady state never grows).
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.scratch.reserve(additional);
    }

    /// Whether nothing has been written yet.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.scratch.is_empty()
    }

    /// Finish into a [`DocText::Pooled`] text doc — the streaming equivalent
    /// of [`DocArena::text_pooled`] (same eager width policy).
    #[inline]
    pub fn finish_text(self) -> DocId {
        let id = self.arena.text_pooled(&self.scratch);
        self.park();
        id
    }

    /// Finish into a [`DocNode::MultilineText`] doc — the streaming equivalent
    /// of [`DocArena::multiline_text`] (body must be framed the same way).
    #[inline]
    pub fn finish_multiline_text(self) -> DocId {
        let id = self.arena.multiline_text(&self.scratch);
        self.park();
        id
    }

    /// Finish into a literal-break [`DocNode::MultilineText`] doc — the streaming
    /// equivalent of [`DocArena::multiline_text_literal`].
    #[inline]
    pub fn finish_multiline_text_literal(self) -> DocId {
        let id = self.arena.multiline_text_literal(&self.scratch);
        self.park();
        id
    }

    /// Return the (cleared) scratch to the arena, retaining capacity.
    #[inline]
    fn park(mut self) {
        self.scratch.clear();
        self.arena.pool_scratch.set(self.scratch);
    }
}

impl std::fmt::Write for PoolTextWriter<'_> {
    #[inline]
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.scratch.push_str(s);
        Ok(())
    }
}

#[cfg(test)]
mod prelude_tests {
    use super::{
        DocArena, DocId, DocNode, DocText, PRELUDE, PRELUDE_BREAK_PARENT, PRELUDE_EMPTY,
        PRELUDE_FLOW_PROBE_END, PRELUDE_FLUSH_BREAK, PRELUDE_LINE, PRELUDE_LINE_SUFFIX_BOUNDARY,
        PRELUDE_STATICS, pooled_text_width, prelude_index,
    };
    use crate::doc::LineKind;

    /// The match and the table are two spellings of one list: every entry answers
    /// its own index, nothing else answers at all, and no entry repeats.
    #[test]
    fn prelude_index_is_the_tables_order() {
        for (i, &(s, _)) in PRELUDE.iter().enumerate() {
            assert_eq!(prelude_index(s), Some(PRELUDE_STATICS + i as u32), "{s:?}");
            assert_eq!(
                PRELUDE.iter().filter(|&&(t, _)| t == s).count(),
                1,
                "{s:?} repeats"
            );
        }
        for s in ["", "const", "await ", "import ", "x", "((", "\n"] {
            assert_eq!(prelude_index(s), None, "{s:?}");
        }
    }

    /// The stated widths are what the measure would have computed.
    #[test]
    fn prelude_widths_match_the_measure() {
        for &(s, width) in PRELUDE {
            assert_eq!(pooled_text_width(s), width, "{s:?}");
        }
    }

    /// Every document opens with the prelude at fixed ids: `text(member)` returns the
    /// seeded node, before and after a `reset()`, and never allocates.
    #[test]
    fn prelude_ids_are_fixed_across_resets() {
        let mut arena = DocArena::new();
        let before = arena.nodes.borrow().len();
        let statics = PRELUDE_STATICS as usize;
        assert_eq!(before, statics + PRELUDE.len());
        for (i, &(s, width)) in PRELUDE.iter().enumerate() {
            assert_eq!(arena.text(s), DocId(PRELUDE_STATICS + i as u32));
            assert!(matches!(
                arena.nodes.borrow()[statics + i],
                DocNode::Text(DocText::Static(t, w)) if t == s && w == width
            ));
        }
        assert_eq!(arena.nodes.borrow().len(), before);
        arena.text("const");
        arena.reset();
        assert_eq!(arena.nodes.borrow().len(), statics + PRELUDE.len());
        assert_eq!(arena.text(","), DocId(PRELUDE_STATICS + 4));
    }

    /// The singletons are the prelude's head, in seeding order, each builder its id.
    #[test]
    fn singletons_are_the_preludes_head() {
        let arena = DocArena::new();
        let nodes = arena.nodes.borrow();
        assert_eq!(arena.empty(), DocId(PRELUDE_EMPTY));
        assert!(matches!(
            nodes[PRELUDE_EMPTY as usize],
            DocNode::Text(DocText::Static("", 0))
        ));
        for kind in [
            LineKind::Normal,
            LineKind::Soft,
            LineKind::Hard,
            LineKind::Literal,
        ] {
            let id = PRELUDE_LINE + kind as u32;
            assert!(matches!(nodes[id as usize], DocNode::Line(k) if k == kind));
        }
        assert_eq!(arena.line(), DocId(PRELUDE_LINE + LineKind::Normal as u32));
        assert_eq!(
            arena.softline(),
            DocId(PRELUDE_LINE + LineKind::Soft as u32)
        );
        assert_eq!(
            arena.hardline(),
            DocId(PRELUDE_LINE + LineKind::Hard as u32)
        );
        assert_eq!(
            arena.literalline(),
            DocId(PRELUDE_LINE + LineKind::Literal as u32)
        );
        assert_eq!(
            arena.line_suffix_boundary(),
            DocId(PRELUDE_LINE_SUFFIX_BOUNDARY)
        );
        assert!(matches!(
            nodes[PRELUDE_LINE_SUFFIX_BOUNDARY as usize],
            DocNode::LineSuffixBoundary
        ));
        assert_eq!(arena.break_parent(), DocId(PRELUDE_BREAK_PARENT));
        assert!(matches!(
            nodes[PRELUDE_BREAK_PARENT as usize],
            DocNode::BreakParent
        ));
        assert_eq!(arena.flush_break(), DocId(PRELUDE_FLUSH_BREAK));
        assert!(matches!(
            nodes[PRELUDE_FLUSH_BREAK as usize],
            DocNode::FlushBreak
        ));
        assert_eq!(arena.flow_probe_end_node(), DocId(PRELUDE_FLOW_PROBE_END));
        assert!(matches!(
            nodes[PRELUDE_FLOW_PROBE_END as usize],
            DocNode::FlowProbeEnd
        ));
        assert_eq!(PRELUDE_FLOW_PROBE_END + 1, PRELUDE_STATICS);
    }
}

#[cfg(test)]
mod pooled_text_width_tests {
    use super::{TEXT_WIDTH_FIRST_LINE_MASK, TEXT_WIDTH_NEWLINE_FLAG, pooled_text_width};
    use crate::config::TAB_WIDTH;
    use crate::printing::visual_width;

    /// The width, spelled out independently of [`pooled_text_width`]: probe for a
    /// newline, then measure. This is the oracle — the fused single-pass scan must
    /// agree with it on every input.
    ///
    /// It has to be graded here because **no corpus can grade it**. A width only
    /// changes the output once it crosses the print width, so an arithmetic slip
    /// on a rare byte (a tab, a control char) leaves every formatted file
    /// byte-identical and sails through the fixtures and any size of format/wire
    /// diff. Verified: a one-column error in the tab arm is invisible to all of
    /// them and caught only here.
    fn reference(s: &str) -> u16 {
        let clamp = |w: usize| w.min(TEXT_WIDTH_FIRST_LINE_MASK as usize) as u16;
        match s.split_once('\n') {
            Some((first, _)) => TEXT_WIDTH_NEWLINE_FLAG | clamp(visual_width(first, TAB_WIDTH)),
            None => clamp(visual_width(s, TAB_WIDTH)),
        }
    }

    fn assert_agrees(s: &str) {
        assert_eq!(
            pooled_text_width(s),
            reference(s),
            "pooled_text_width disagrees with the reference on {s:?}"
        );
    }

    #[test]
    fn agrees_on_exhaustive_short_strings() {
        // Every string of length 0-3 over an alphabet spanning each arm of the
        // scan: plain ASCII, the two special ASCII bytes, a control char, DEL,
        // and multi-byte UTF-8 (2-, 3- and 4-byte, plus a combining mark and a
        // ZWJ — the clusters that can cross an ASCII boundary).
        let alphabet = [
            "a", "Z", "0", "-", " ", "\t", "\n", "\r", "\x00", "\x1b", "\x7f", "é", "中", "🎉",
            "\u{0301}", "\u{200d}", "\u{fe0f}", "\u{00a0}",
        ];
        assert_agrees("");
        for a in alphabet {
            assert_agrees(a);
            for b in alphabet {
                assert_agrees(&format!("{a}{b}"));
                for c in alphabet {
                    assert_agrees(&format!("{a}{b}{c}"));
                }
            }
        }
    }

    #[test]
    fn agrees_on_realistic_and_boundary_inputs() {
        for s in [
            "color",
            "--custom-property",
            "rgb(12 34 56 / 0.5)",
            "\tindented",
            "a\tb\tc",
            "line one\nline two",
            // A newline positioned AFTER the first non-ASCII byte: the fast
            // path bails to the cold arm mid-scan, which must still find it.
            "é\nafter",
            "中\ttab-after-multibyte",
            // A combining mark on an ASCII base — the cluster starts on the
            // byte the fast path already counted, so the cold arm has to
            // re-measure the whole slice, not the remainder.
            "e\u{0301}x",
            "\u{200d}",
            "1\u{fe0f}\u{20e3}",
            "👨\u{200d}👩\u{200d}👧",
        ] {
            assert_agrees(s);
        }
    }

    #[test]
    fn agrees_at_the_clamp_boundary() {
        // A single-line text wider than the flag must clamp, not set
        // TEXT_WIDTH_NEWLINE_FLAG or wrap; a first line that wide clamps under
        // the flag the same way.
        for len in [
            TEXT_WIDTH_NEWLINE_FLAG as usize - 2,
            TEXT_WIDTH_NEWLINE_FLAG as usize - 1,
            TEXT_WIDTH_NEWLINE_FLAG as usize,
            TEXT_WIDTH_NEWLINE_FLAG as usize + 5,
            u16::MAX as usize + 5,
        ] {
            let ascii = "a".repeat(len);
            assert_agrees(&ascii);
            assert_eq!(pooled_text_width(&ascii) & TEXT_WIDTH_NEWLINE_FLAG, 0);
            // Tabs multiply the width, so a far shorter run also clamps.
            let tabs = "\t".repeat(len);
            assert_agrees(&tabs);
            let first_line = format!("{ascii}\nb");
            assert_agrees(&first_line);
            assert!(pooled_text_width(&first_line) & TEXT_WIDTH_NEWLINE_FLAG != 0);
        }
    }

    #[test]
    fn agrees_on_long_ascii_runs() {
        // The length range where the replaced shape's SIMD scans were at their
        // best — the word-at-a-time scan must still agree there.
        for len in [31, 32, 33, 63, 64, 65, 127, 128, 256, 1000] {
            assert_agrees(&"x".repeat(len));
            assert_agrees(&format!("{}\t{}", "x".repeat(len / 2), "y".repeat(len / 2)));
            assert_agrees(&format!("{}\n{}", "x".repeat(len / 2), "y".repeat(len / 2)));
            assert_agrees(&format!("{}é{}", "x".repeat(len / 2), "y".repeat(len / 2)));
        }
    }

    /// Every class byte at every alignment of a scan whose stride is **eight**.
    ///
    /// The exhaustive test above tops out at three characters, so it never entered
    /// a word loop at all — and a word loop's failure modes are all positional: a
    /// hit in the last lane, a hit one byte past the final whole word, a class byte
    /// that only the scalar tail can see, a borrow crossing a lane. The lengths run
    /// past three whole words and the special byte visits **every** index in each,
    /// which is the only shape that grades the tail against the word loop and both
    /// against the oracle.
    #[test]
    fn agrees_with_a_class_byte_at_every_alignment() {
        // One representative per arm of the scan, plus the bytes adjacent to the
        // needles in value order (0x09 / 0x0a are the needles; 0x0b and 0x7f must
        // NOT fire, 0x80 must).
        let specials = [
            "\t", "\n", "\r", "\x0b", "\x00", "\x7f", "\u{80}", "é", "中", "🎉", "\u{0301}",
        ];
        for len in 0..=40usize {
            for special in specials {
                for at in 0..=len {
                    let mut v = String::with_capacity(len + 4);
                    v.push_str(&"x".repeat(at));
                    v.push_str(special);
                    v.push_str(&"y".repeat(len - at));
                    assert_agrees(&v);
                }
            }
        }
    }

    /// Two class bytes in one slice, at every pair of alignments across a word
    /// boundary — the shape that grades the *resume*: the special arm re-enters the
    /// scan at `i + 1` after a tab, and a second hit in the same word (or in the
    /// scalar tail after a hit in the last word) is where an off-by-one lives.
    #[test]
    fn agrees_with_two_class_bytes_at_every_alignment() {
        for len in 0..=20usize {
            for (a, b) in [
                ("\t", "\t"),
                ("\t", "\n"),
                ("\n", "\t"),
                ("\t", "é"),
                ("é", "\t"),
                ("é", "\n"),
                ("\t", "🎉"),
            ] {
                for i in 0..=len {
                    for j in i..=len {
                        let mut v = String::new();
                        v.push_str(&"x".repeat(i));
                        v.push_str(a);
                        v.push_str(&"y".repeat(j - i));
                        v.push_str(b);
                        v.push_str(&"z".repeat(len - j));
                        assert_agrees(&v);
                    }
                }
            }
        }
    }
}

/// [`source_span_width`] must answer exactly what [`pooled_text_width`] answers for the
/// same text — and the two reach it differently, so only a differential says so.
///
/// **The span form reads bytes it must not believe**, which is a failure surface the
/// slice form does not have and no corpus can be trusted to cover: the word loop reads
/// eight bytes of the *document* at every step, so a `\n` sitting one byte past the span
/// (the single most common neighbour an identifier has) is in the register when the
/// verdict is taken. Believing it turns a plain name's width into a flagged first-line width,
/// which changes a fits verdict and nothing else — no fixture, no format diff, no wire
/// diff can see it. So the neighbour is enumerated here rather than sampled: every class
/// byte at every distance past the span's end, at every alignment of the span's start.
#[cfg(test)]
mod source_span_width_tests {
    use super::{clamp_text_width, pooled_text_width, source_span_width};
    use crate::span::Span;

    /// One representative per arm of the class, plus two bytes adjacent to the needles
    /// in value order (`0x0b` and `0x7f` must NOT fire, `0x80` must).
    const SPECIALS: [&str; 8] = ["\t", "\n", "\x0b", "\x7f", "\u{80}", "é", "中", "🎉"];

    #[track_caller]
    fn assert_agrees(host: &str, start: usize, end: usize) {
        let span = Span::new(start as u32, end as u32);
        assert_eq!(
            source_span_width(span, host),
            pooled_text_width(&host[start..end]),
            "source_span_width disagrees on {:?} of {host:?}",
            &host[start..end],
        );
    }

    /// The neighbour test: a class byte just past the span, which the word read sees and
    /// the answer must not. Every distance past the end from 0 (inside — a control, so
    /// the width really does change) through 8 (a whole word away), at every start
    /// alignment, so the byte visits every lane of the read.
    #[test]
    fn ignores_a_class_byte_past_the_span_end() {
        for special in SPECIALS {
            for align in 0..9usize {
                for len in 0..=17usize {
                    for gap in 0..=8usize {
                        let host = format!(
                            "{}{}{}{}{}",
                            "a".repeat(align),
                            "x".repeat(len),
                            "b".repeat(gap),
                            special,
                            "c".repeat(9),
                        );
                        assert_agrees(&host, align, align + len);
                    }
                }
            }
        }
    }

    /// [`DocArena::source_span_plain`] does not measure at all, so its claim is an
    /// EQUATION and this is where it is checked: wherever the span holds no
    /// width-relevant byte, the measured width and the clamped byte length must be the
    /// same number. Walked at every length and every start alignment, with a class byte
    /// on each side of the span so the equation is not read off an all-`x` document.
    #[test]
    fn the_plain_width_equals_the_measured_one_wherever_the_claim_holds() {
        for special in SPECIALS {
            for align in 0..9usize {
                for len in 0..=17usize {
                    let host = format!(
                        "{special}{}{}{}{special}",
                        "a".repeat(align),
                        "x".repeat(len),
                        "c".repeat(9),
                    );
                    let start = special.len() + align;
                    let span = Span::new(start as u32, (start + len) as u32);
                    assert_eq!(
                        source_span_width(span, &host),
                        clamp_text_width(len),
                        "the plain width disagrees with the measured one on {:?}",
                        &host[start..start + len],
                    );
                }
            }
        }
    }

    /// The mirror, and the one that makes the equation above a TEST rather than a
    /// tautology: with a class byte inside the span the two answers must DIFFER, so a
    /// caller that claims `plain_ascii` over such a span is claiming something false.
    ///
    /// Restricted to the members of the class — [`SPECIALS`] deliberately also carries
    /// two neighbours in value order (`0x0b`, `0x7f`) that are one column each, and a
    /// span holding one of those really does measure its own length.
    #[test]
    fn the_plain_width_differs_wherever_the_claim_fails() {
        for special in SPECIALS {
            if !special.bytes().any(crate::printing::is_width_relevant) {
                continue;
            }
            for align in 0..9usize {
                for len in 1..=17usize {
                    for at in 0..len {
                        let host = format!(
                            "{}{}{special}{}{}",
                            "a".repeat(align),
                            "x".repeat(at),
                            "y".repeat(len - 1 - at),
                            "zzzzzzzzzzzz",
                        );
                        let end = align + at + special.len() + (len - 1 - at);
                        let span = Span::new(align as u32, end as u32);
                        assert_ne!(
                            source_span_width(span, &host),
                            clamp_text_width(end - align),
                            "the plain width would have been right on {:?}",
                            &host[align..end],
                        );
                    }
                }
            }
        }
    }

    /// A class byte INSIDE the span, at every alignment — the mirror of the above, and
    /// the case a too-eager `end` clamp would break.
    #[test]
    fn finds_a_class_byte_inside_the_span_at_every_alignment() {
        for special in SPECIALS {
            for align in 0..9usize {
                for len in 1..=17usize {
                    for at in 0..len {
                        let host = format!(
                            "{}{}{}{}{}",
                            "a".repeat(align),
                            "x".repeat(at),
                            special,
                            "y".repeat(len - 1 - at),
                            "zzzzzzzzzzzz",
                        );
                        let end = align + at + special.len() + (len - 1 - at);
                        assert_agrees(&host, align, end);
                    }
                }
            }
        }
    }

    /// A span ending at (or within eight bytes of) the document's end — the one place no
    /// word is readable and the scalar class test still runs. `72` spans a 1,666-file
    /// TypeScript run land here, so it is rare and must still be exact.
    #[test]
    fn agrees_within_a_word_of_the_documents_end() {
        for special in SPECIALS {
            for tail in 0..9usize {
                for len in 0..=12usize {
                    let host = format!("{}{}{}", "x".repeat(len), special, "y".repeat(tail));
                    let host_len = host.len();
                    // Every span that reaches into the last word, including the empty
                    // one at the very end.
                    for start in 0..=len {
                        assert_agrees(&host, start, host_len);
                        assert_agrees(&host, start, len);
                    }
                }
            }
        }
    }

    /// Spans long enough to run several whole words, so the loop's advance is graded
    /// against the slice form and not only its first read.
    #[test]
    fn agrees_on_multi_word_spans() {
        let host = format!(
            "{}\t{}é{}\n{}",
            "x".repeat(40),
            "y".repeat(40),
            "z".repeat(40),
            "w".repeat(40),
        );
        let host_len = host.len();
        for start in 0..host_len {
            if !host.is_char_boundary(start) {
                continue;
            }
            for end in [start, start + 1, start + 7, start + 8, start + 33, host_len] {
                if end <= host_len && host.is_char_boundary(end) {
                    assert_agrees(&host, start, end);
                }
            }
        }
    }

    /// A non-empty span that splits a multi-byte character panics exactly where the `str`
    /// index always did — it always contains a byte at or above `0x80`, so it reaches the
    /// cold arm and its `span.extract`. Both ends are graded: a split `end` (the lead
    /// byte is inside) and a split `start` (the start is itself a continuation byte).
    #[test]
    fn a_span_off_a_char_boundary_still_panics() {
        for span in [Span::new(0, 1), Span::new(1, 3)] {
            let caught = std::panic::catch_unwind(|| {
                source_span_width(span, "é_padding_padding_padding");
            });
            assert!(caught.is_err(), "{span:?} should still panic");
        }
    }

    /// A span past the document's end fails at the same `str` index it always did.
    #[test]
    #[should_panic(expected = "out of bounds")]
    fn a_span_past_the_documents_end_still_panics() {
        source_span_width(Span::new(1, 99), "short");
    }
}

#[cfg(test)]
mod render_indent_tests {
    //! Equivalence test for [`RenderIndent`] — its incremental `(tabs,
    //! pending_aligns, align_spaces)` state must reproduce Prettier's queue-based
    //! `generateIndent` (`document/printer/indent.js`) under `useTabs` exactly.
    //!
    //! **No corpus can grade this.** A sub-tab alignment only changes the output
    //! at a trailing closing delimiter, and there only the tab-vs-space
    //! *representation* (equal visual width at `tabWidth = 2`), so an arithmetic
    //! slip in the round-up or the space count leaves every formatted file
    //! byte-identical and sails through the fixtures and any size of diff. This
    //! reference — Prettier's algorithm spelled out from scratch — is the only
    //! gate with power over the fact. Corruption-verify any change to
    //! [`RenderIndent`]'s ops by breaking one and watching an assertion fail.
    use super::RenderIndent;
    use crate::config::TAB_WIDTH;

    #[derive(Clone, Copy, Debug)]
    enum Op {
        /// Prettier's `makeIndent` (queue `INDENT`).
        Indent,
        /// Prettier's numeric `makeAlign` (queue `WIDTH { width: n }`).
        Align(u32),
    }

    /// Prettier's `generateIndent` for a queue of INDENT/WIDTH commands, in
    /// `useTabs` mode — spelled out independently of [`RenderIndent`]. Returns
    /// the whitespace `value` and its column `length`.
    fn reference(queue: &[Op]) -> (String, usize) {
        let mut value = String::new();
        let mut length = 0usize;
        let mut last_tabs = 0usize;
        let mut last_spaces = 0usize;
        for op in queue {
            match *op {
                Op::Indent => {
                    // flush() -> flushTabs() (useTabs), then addTabs(1).
                    if last_tabs > 0 {
                        for _ in 0..last_tabs {
                            value.push('\t');
                        }
                        length += TAB_WIDTH * last_tabs;
                    }
                    last_tabs = 0;
                    last_spaces = 0;
                    value.push('\t');
                    length += TAB_WIDTH;
                }
                Op::Align(n) => {
                    last_tabs += 1;
                    last_spaces += n as usize;
                }
            }
        }
        // Final flushSpaces(): emit lastSpaces, discard lastTabs.
        for _ in 0..last_spaces {
            value.push(' ');
        }
        length += last_spaces;
        (value, length)
    }

    fn apply(queue: &[Op]) -> RenderIndent {
        let mut indent = RenderIndent::default();
        for op in queue {
            indent = match *op {
                Op::Indent => indent.indented(),
                Op::Align(n) => indent.aligned(n),
            };
        }
        indent
    }

    fn whitespace(indent: RenderIndent) -> String {
        let mut s = String::new();
        for _ in 0..indent.tabs() {
            s.push('\t');
        }
        for _ in 0..indent.trailing_align_spaces() {
            s.push(' ');
        }
        s
    }

    fn assert_agrees(queue: &[Op]) {
        let (value, length) = reference(queue);
        let indent = apply(queue);
        assert_eq!(
            whitespace(indent),
            value,
            "RenderIndent whitespace disagrees with generateIndent on {queue:?}"
        );
        assert_eq!(
            indent.column(TAB_WIDTH),
            length,
            "RenderIndent column disagrees with generateIndent length on {queue:?}"
        );
    }

    /// Exhaustively grade every op sequence of length 0..=5 over
    /// {Indent, align(1), align(2)} — 3^0 + … + 3^5 = 364 sequences. Covers the
    /// two behaviors that matter: a trailing align run renders as literal spaces
    /// (closing delimiter), and an align run flushed by a following indent rounds
    /// up to whole tabs (content line).
    #[test]
    fn render_indent_matches_prettier_generate_indent() {
        const OPS: [Op; 3] = [Op::Indent, Op::Align(1), Op::Align(2)];
        let mut queue: Vec<Op> = Vec::new();
        fn recurse(queue: &mut Vec<Op>, depth: usize, ops: &[Op], f: &dyn Fn(&[Op])) {
            f(queue);
            if depth == 0 {
                return;
            }
            for &op in ops {
                queue.push(op);
                recurse(queue, depth - 1, ops, f);
                queue.pop();
            }
        }
        recurse(&mut queue, 5, &OPS, &assert_agrees);
    }

    /// The union-member closing-delimiter case, concretely: `level(1)` then a
    /// trailing `align(2)` renders as `1 tab + 2 spaces` (the fix — Prettier's
    /// output), never `2 tabs`.
    #[test]
    fn trailing_align_is_literal_spaces() {
        let indent = RenderIndent::level(1).aligned(2);
        assert_eq!(whitespace(indent), "\t  ");
        assert_eq!(indent.column(TAB_WIDTH), TAB_WIDTH + 2);
    }

    /// An align followed by an indent rounds up to a whole tab (content line),
    /// so member bodies stay byte-identical to the pre-fix whole-tab output.
    #[test]
    fn align_then_indent_rounds_up_to_tab() {
        let indent = RenderIndent::level(1).aligned(2).indented();
        assert_eq!(whitespace(indent), "\t\t\t");
        assert_eq!(indent.column(TAB_WIDTH), 3 * TAB_WIDTH);
    }

    /// Grade `indented()` / `dedented()` on pure whole-tab queues against
    /// Prettier's queue model: a dedent is `queue.slice(0, -1)`, which on a
    /// pure-INDENT queue is exactly popping one tab (saturating at 0, since
    /// slicing an empty queue leaves it empty). Exhaustive over every
    /// {Indent, Dedent} sequence of length 0..=6 — no align, so tsv's
    /// dedent-never-crosses-an-align-run invariant is respected.
    #[test]
    fn dedent_pops_one_tab_like_prettier_queue_slice() {
        #[derive(Clone, Copy, Debug)]
        enum TabOp {
            Indent,
            Dedent,
        }
        // Prettier queue depth: Indent pushes, Dedent slices off the last.
        fn ref_depth(ops: &[TabOp]) -> usize {
            let mut depth = 0usize;
            for op in ops {
                match op {
                    TabOp::Indent => depth += 1,
                    TabOp::Dedent => depth = depth.saturating_sub(1),
                }
            }
            depth
        }
        fn apply_tabs(ops: &[TabOp]) -> RenderIndent {
            let mut indent = RenderIndent::default();
            for op in ops {
                indent = match op {
                    TabOp::Indent => indent.indented(),
                    TabOp::Dedent => indent.dedented(),
                };
            }
            indent
        }
        fn recurse(queue: &mut Vec<TabOp>, depth: usize, f: &dyn Fn(&[TabOp])) {
            f(queue);
            if depth == 0 {
                return;
            }
            for op in [TabOp::Indent, TabOp::Dedent] {
                queue.push(op);
                recurse(queue, depth - 1, f);
                queue.pop();
            }
        }
        let mut queue: Vec<TabOp> = Vec::new();
        recurse(&mut queue, 6, &|ops| {
            let indent = apply_tabs(ops);
            let depth = ref_depth(ops);
            assert_eq!(
                indent.tabs(),
                depth,
                "dedent tab depth disagrees on {ops:?}"
            );
            assert_eq!(indent.column(TAB_WIDTH), depth * TAB_WIDTH);
            assert_eq!(indent.trailing_align_spaces(), 0);
        });
    }

    /// `reset_to_level` (the `AlignRoot` node / template-literal root reset)
    /// clears any pending align run and sets an absolute whole-tab level —
    /// Prettier's root reset emptying the indent queue.
    #[test]
    fn reset_to_level_clears_pending_align_run() {
        // A pending trailing align run, then reset to root (level 0) → empty.
        let indent = RenderIndent::level(3)
            .aligned(2)
            .aligned(1)
            .reset_to_level(0);
        assert_eq!(indent, RenderIndent::default());
        assert_eq!(whitespace(indent), "");
        assert_eq!(indent.column(TAB_WIDTH), 0);
        // Reset to a nonzero absolute level is pure tabs; the run is discarded.
        let indent = RenderIndent::level(1).aligned(2).reset_to_level(2);
        assert_eq!(whitespace(indent), "\t\t");
        assert_eq!(indent.trailing_align_spaces(), 0);
        assert_eq!(indent.column(TAB_WIDTH), 2 * TAB_WIDTH);
    }
}

#[cfg(test)]
mod inline_sibling_line_group_tests {
    //! The inline-sibling wrap producer/matcher must stay in lockstep. The producer
    //! ([`super::DocArena::inline_sibling_line_group`]) lives here in `tsv_lang`; its consumer
    //! ([`super::DocArena::strip_leading_line_group`], the after-element fold's matcher) is asked a
    //! crate away in `tsv_svelte`. A silent shape drift returns `None` there and reintroduces the
    //! stray-space non-idempotency the fold exists to prevent — invisible until an authoring corpus
    //! hits it. This round-trip is the guard the ~600-line producer↔matcher gap otherwise lacks.
    use super::DocArena;

    #[test]
    fn strip_leading_line_group_round_trips() {
        let a = DocArena::new();
        let x = a.text("x");
        assert_eq!(
            a.strip_leading_line_group(a.inline_sibling_line_group(x)),
            Some(x),
            "strip_leading_line_group must be the exact inverse of inline_sibling_line_group",
        );
    }

    #[test]
    fn strip_leading_line_group_ex_round_trips_both_forms() {
        let a = DocArena::new();
        let x = a.text("x");
        assert_eq!(
            a.strip_leading_line_group_ex(a.inline_sibling_line_group(x)),
            Some((x, false)),
            "the plain wrap strips to its inner doc and reports NOT held",
        );
        assert_eq!(
            a.strip_leading_line_group_ex(a.inline_sibling_line_group_held(x)),
            Some((x, true)),
            "the held wrap strips to its inner doc and reports held — a rejoin must re-wrap held",
        );
        // The plain matcher strips the held wrap too (it is the same shape to every reader that
        // does not re-wrap), so a caller that only needs `X` sees no difference.
        assert_eq!(
            a.strip_leading_line_group(a.inline_sibling_line_group_held(x)),
            Some(x)
        );
    }

    #[test]
    fn strip_leading_line_group_rejects_other_shapes() {
        let a = DocArena::new();
        let x = a.text("x");
        // A bare element (no wrap) and a group whose lead is not a boundary line are both `None` —
        // the fold then keeps them intact rather than stripping a line that was never there.
        assert_eq!(a.strip_leading_line_group(x), None);
        let no_lead_line = a.group(a.concat(&[a.text("a"), a.line()]));
        assert_eq!(a.strip_leading_line_group(no_lead_line), None);
        // A forced-break group of the right shape is also rejected (the wrap is non-breaking).
        let broken = a.group_break(a.concat(&[a.line(), x]));
        assert_eq!(a.strip_leading_line_group(broken), None);
    }
}

#[cfg(test)]
mod welded_marker_burial_tests {
    //! [`super::DocArena::welded_entry`] reads only the top node, so ANY builder that wraps a
    //! marked doc can bury the marker: the walk then stops one entry short, and the run stands
    //! and tears its last element open instead of travelling — invisible to every
    //! idempotency-shaped gate, since the torn form is its own fixed point. A retired sibling
    //! join (`group([marked, line])`) was the historical instance, and had to re-hoist the
    //! marker's flags onto the wrapper to stay visible; the tripwire below is what keeps the
    //! next such builder from repeating it.
    use super::{DocArena, DocContext, WeldedEntry};

    fn marker(a: &DocArena) -> super::DocId {
        a.with_context(
            a.text("el"),
            DocContext::default()
                .with_glued_lead(true)
                .with_glued_atom(true),
        )
    }

    /// The burial tripwire must FIRE on the historical bug shape — a marker wrapped in a bare
    /// `group([marked, line])` with its flags NOT re-hoisted.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "buried welded-run marker")]
    fn burial_tripwire_fires_on_a_bare_wrapping_join() {
        let a = DocArena::new();
        let buried = a.group(a.concat(&[marker(&a), a.line()]));
        assert!(matches!(a.welded_entry(buried), WeldedEntry::NotGlued));
        a.debug_check_buried_welded_marker(buried);
    }

    /// …and stay QUIET on the legitimate shapes: the hoisting join itself (never classified
    /// `NotGlued`, but the descent must also not fire through it), a marker sitting behind a
    /// `Fill`'s items (a glued boundary in some deeper fill — the descent stops at the `Fill`),
    /// and plain markerless content.
    #[cfg(debug_assertions)]
    #[test]
    fn burial_tripwire_quiet_on_legitimate_nesting() {
        let a = DocArena::new();
        // A marker deeper in a fill: first structural child chain ends at the Fill node.
        let nested = a.group(a.fill(&[marker(&a), a.line(), a.text("word")]));
        a.debug_check_buried_welded_marker(nested);
        // Ordinary content, arbitrarily wrapped.
        let plain = a.group(a.concat(&[a.indent(a.text("x")), a.line()]));
        a.debug_check_buried_welded_marker(plain);
        // A WithContext without the marker flag descends without firing. The flag has to be a
        // BUILD-side one — the CSS printer's `trailing_reserve` marker is the real instance — since
        // `with_context`'s own tripwire holds the render-side flags to a `Fill`.
        let flagged = a.with_context(a.text("y"), DocContext::reserving(4));
        a.debug_check_buried_welded_marker(a.group(a.concat(&[flagged, a.line()])));
    }
}

#[cfg(test)]
mod as_source_span_text_tests {
    use super::DocArena;
    use crate::Span;

    /// `Some` for a doc that is one source-span text, measured or claimed plain, and for
    /// nothing else — not a static or pooled text, not the will-break-opaque verbatim
    /// span, not a concat holding a source span.
    #[test]
    fn only_a_lone_source_span_answers() {
        let source = "abc def";
        let span = Span { start: 0, end: 3 };
        let a = DocArena::for_source(source);

        assert_eq!(
            a.as_source_span_text(a.source_span(span, source)),
            Some(span)
        );
        assert_eq!(a.as_source_span_text(a.source_span_plain(span)), Some(span));

        assert_eq!(a.as_source_span_text(a.text("abc")), None);
        assert_eq!(a.as_source_span_text(a.text_pooled("abc")), None);
        assert_eq!(
            a.as_source_span_text(a.verbatim_source_span(span, source)),
            None
        );
        let pair = a.concat(&[a.source_span_plain(span), a.text(";")]);
        assert_eq!(a.as_source_span_text(pair), None);
    }
}

#[cfg(test)]
mod leading_comment_break_tests {
    //! [`super::DocArena::leading_comment_break`] reads a built doc's leading edge, and a
    //! restricted production's ASI safety rests on it: each layout the doc can take is
    //! followed to its first code token. These pin the readings a caller branches on.
    use super::{DocArena, LeadingBreak, LeadingEdge};
    use crate::Span;

    const SOURCE: &str = "/* c */ a /*\n*/ // d";

    fn comment(a: &DocArena) -> super::DocId {
        a.source_span(Span { start: 0, end: 7 }, SOURCE)
    }

    fn code(a: &DocArena) -> super::DocId {
        a.text("a")
    }

    fn edge(a: &DocArena, doc: super::DocId) -> LeadingEdge {
        a.leading_comment_break(doc, SOURCE)
    }

    #[test]
    fn code_first_reads_nothing() {
        let a = DocArena::new();
        let doc = a.concat(&[code(&a), comment(&a), a.hardline(), code(&a)]);
        assert_eq!(edge(&a, doc), LeadingEdge::NONE);
    }

    #[test]
    fn a_line_after_a_comment_is_soft_in_an_open_group_and_ungrouped_outside_one() {
        let a = DocArena::new();
        let run = a.concat(&[comment(&a), a.line(), code(&a)]);
        let outside = edge(&a, run);
        assert_eq!(outside.brk, LeadingBreak::Soft);
        assert!(outside.ungrouped);
        assert_eq!(outside.first_comment, Some(Span { start: 0, end: 7 }));

        let grouped = edge(&a, a.group(run));
        assert_eq!(grouped.brk, LeadingBreak::Soft);
        assert!(!grouped.ungrouped);
    }

    #[test]
    fn a_forced_group_or_a_hard_line_or_a_line_comment_is_hard() {
        let a = DocArena::new();
        let soft = a.concat(&[comment(&a), a.line(), code(&a)]);
        let forced = a.group(a.concat(&[soft, a.break_parent()]));
        assert_eq!(edge(&a, forced).brk, LeadingBreak::Hard);
        assert_eq!(edge(&a, a.group_break(soft)).brk, LeadingBreak::Hard);

        let hard = a.concat(&[comment(&a), a.hardline(), code(&a)]);
        assert_eq!(edge(&a, hard).brk, LeadingBreak::Hard);

        let line_comment = a.source_span(Span { start: 16, end: 20 }, SOURCE);
        assert_eq!(
            edge(&a, a.concat(&[line_comment, code(&a)])).brk,
            LeadingBreak::Hard
        );
    }

    #[test]
    fn a_multiline_block_is_an_asi_terminator_however_it_is_followed() {
        let a = DocArena::new();
        let block = a.source_span(Span { start: 10, end: 15 }, SOURCE);
        let e = edge(&a, a.concat(&[block, a.text(" "), code(&a)]));
        assert_eq!(e.brk, LeadingBreak::None);
        assert!(e.multiline);
        assert_eq!(e.asi(), LeadingBreak::Hard);

        let pooled = a.multiline_text("/*\n*/");
        assert_eq!(
            edge(&a, a.concat(&[pooled, code(&a)])).asi(),
            LeadingBreak::Hard
        );
    }

    #[test]
    fn a_line_ahead_of_every_comment_is_read_apart() {
        let a = DocArena::new();
        let e = edge(&a, a.group(a.concat(&[a.softline(), code(&a)])));
        assert_eq!(e.brk, LeadingBreak::None);
        assert_eq!(e.bare, LeadingBreak::Soft);
        assert_eq!(e.asi(), LeadingBreak::Soft);
    }

    /// The conditional pair a soft leading edge takes: its `(` opens the broken layout, and
    /// the flat one prints every line of the group flat, so neither ends a line ahead of
    /// the first token.
    #[test]
    fn the_conditional_pair_holds_its_soft_edge() {
        let a = DocArena::new();
        let inner = a.group(a.concat(&[comment(&a), a.line(), code(&a)]));
        let pair = a.group(a.concat(&[
            a.if_break(a.text("("), a.empty()),
            a.indent(a.concat(&[a.softline(), inner])),
            a.softline(),
            a.if_break(a.text(")"), a.empty()),
        ]));
        assert_eq!(edge(&a, pair).asi(), LeadingBreak::None);

        // A broken-arm `if_break` keyed on a forced group never takes its flat arm.
        let held = a.group_break(a.concat(&[
            a.if_break(a.text("("), a.empty()),
            comment(&a),
            a.line(),
            code(&a),
        ]));
        assert_eq!(edge(&a, held).asi(), LeadingBreak::None);
    }

    #[test]
    fn every_conditional_group_state_is_read() {
        let a = DocArena::new();
        let flat = a.concat(&[comment(&a), a.text(" "), code(&a)]);
        let broken = a.concat(&[comment(&a), a.hardline(), code(&a)]);
        assert_eq!(
            edge(&a, a.conditional_group(&[flat, broken])).brk,
            LeadingBreak::Hard
        );
        assert_eq!(
            edge(&a, a.conditional_group(&[flat, flat])).brk,
            LeadingBreak::None
        );
    }

    #[test]
    fn a_deferred_run_ahead_of_a_breakable_line_is_a_leading_comment() {
        let a = DocArena::new();
        let suffix = a.line_suffix(comment(&a));
        let doc = a.group(a.concat(&[suffix, a.line(), code(&a)]));
        assert_eq!(edge(&a, doc).brk, LeadingBreak::Soft);
        let flushed = a.concat(&[suffix, a.line_suffix_boundary(), code(&a)]);
        assert_eq!(edge(&a, flushed).brk, LeadingBreak::Hard);
    }

    /// An edge the walk cannot finish within its budget is read as the unsafe one.
    #[test]
    fn an_unfinished_walk_answers_hard() {
        let a = DocArena::new();
        let mut doc = code(&a);
        for _ in 0..5000 {
            doc = a.concat(&[a.text(" "), doc]);
        }
        assert_eq!(edge(&a, doc).brk, LeadingBreak::Hard);
    }
}

#[cfg(test)]
mod walk_memo_tests {
    //! The doc walks outside the renderer memoize what they read of a shared subtree
    //! ([`super::WalkMemo`], which unit tests start from the first visit, and again at the
    //! production threshold). A memo is exact only if a node's answer cannot depend on how
    //! the walk reached it, so each walk is graded on a generated family of docs built
    //! twice: once as a DAG, every repeated subtree one node reached through several
    //! parents, and once as a tree, every use its own copy, where no memo entry is ever
    //! read back. The two must answer alike. `may_end_on_comment_text` reads a per-node
    //! transfer instead of threading its state, so it is also graded against the
    //! state-threading walk it replaced.
    use super::{
        DocArena, DocId, DocNode, FLATTEN_FRAMES, FlattenMode, LastLeaf, PendingRun, RUN_WIDTH_CAP,
        SuffixWalk, WALK_MEMO_AFTER, WalkMemo, WalkStore, test_memo_after,
    };
    use crate::config::{EmbedContext, PRINT_WIDTH};
    use crate::doc::{Mode, arena_fits, arena_print_doc};

    /// A deterministic hash of the generator's seed and a node's path.
    fn mix(seed: u64, path: u64) -> u64 {
        let mut z = seed
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(path)
            .wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A leaf of every kind the walks tell apart: code, comments (inline, multi-line,
    /// deferred), the four line kinds, a flush break, a boundary, nothing. The deferred
    /// runs vary in every way the suffix walk's state records — a `//` or a block, glued or
    /// spaced, a break or a non-text leaf ending the first line, and the width — so two
    /// arms ahead of one shared subtree reach it in states differing in any one field.
    fn leaf(a: &DocArena, pick: u64) -> DocId {
        let wide = |n: u64| "e".repeat(1 + (n % 7) as usize);
        let block = |n: u64| a.text_pooled(&format!("/* {} */", wide(n)));
        let line_comment = |n: u64| a.text_pooled(&format!("// {}", wide(n)));
        let tail = pick >> 5;
        match pick % 18 {
            0 => a.text("a"),
            1 => a.text("/* c */"),
            2 => a.line_suffix(line_comment(tail)),
            3 => a.line_suffix(a.concat(&[a.text(" "), block(tail)])),
            4 => a.line(),
            5 => a.softline(),
            6 => a.hardline(),
            7 => a.flush_break(),
            8 => a.line_suffix_boundary(),
            9 => a.multiline_text("/* f\ng */"),
            10 => a.line_suffix(block(tail)),
            11 => a.line_suffix(a.concat(&[a.line(), block(tail)])),
            12 => a.line_suffix(a.concat(&[line_comment(tail), a.line(), block(tail >> 3)])),
            13 => a.line_suffix(a.concat(&[block(tail), a.if_break(a.text("x"), a.text("y"))])),
            14 => a.line_suffix(a.concat(&[block(tail), a.line(), block(tail >> 3)])),
            15 => a.line_suffix(a.concat(&[block(tail), line_comment(tail >> 3)])),
            16 => a.line_suffix(a.text(" ")),
            _ => a.empty(),
        }
    }

    /// The doc at `path`, `depth` levels deep. Each level uses its child doc twice; with
    /// `shared` the child is built once and both uses name it, without it each use is a
    /// copy built from the same path, so the two forms print and read alike.
    fn build(a: &DocArena, seed: u64, path: u64, depth: u32, shared: bool) -> DocId {
        let h = mix(seed, path);
        if depth == 0 {
            return leaf(a, h);
        }
        let child = |a: &DocArena| build(a, seed, path * 4 + 1, depth - 1, shared);
        let first = child(a);
        let second = if shared { first } else { child(a) };
        let l1 = leaf(a, h >> 8);
        let l2 = leaf(a, h >> 20);
        match (h >> 32) % 6 {
            0 => a.conditional_group(&[a.concat(&[l1, first]), a.concat(&[second, l2])]),
            1 => a.if_break(a.concat(&[first, l1]), a.concat(&[l2, second])),
            2 => a.group(a.concat(&[l1, first, l2, second])),
            3 => a.concat(&[first, l1, second]),
            4 => a.indent(a.if_break(first, second)),
            _ => a.conditional_group(&[first, a.concat(&[l1, second, l2])]),
        }
    }

    /// The state-threading walk `may_end_on_comment_text` took before its transfer form.
    fn last_leaf_reference(a: &DocArena, id: DocId, state: LastLeaf) -> LastLeaf {
        let node = a.get(id).clone();
        match node {
            DocNode::Text(t) => {
                let pool = a.text_pool.borrow();
                let text = super::super::types::resolve_text(&t, Some(""), &pool);
                let text = text.trim_start();
                if text.is_empty() {
                    state
                } else if text.starts_with("/*") || text.starts_with("//") {
                    LastLeaf::Comment
                } else {
                    LastLeaf::Code
                }
            }
            DocNode::MultilineText { .. } => LastLeaf::Comment,
            DocNode::Indent(inner) | DocNode::Dedent(inner) => last_leaf_reference(a, inner, state),
            DocNode::AlignRoot { contents, .. }
            | DocNode::Align { contents, .. }
            | DocNode::IndentIfBreak { contents, .. }
            | DocNode::GatedState { contents, .. } => last_leaf_reference(a, contents, state),
            DocNode::Group {
                contents,
                expanded_states,
                ..
            } => {
                let alts: Vec<DocId> = expanded_states.resolve(&a.borrow_children()).to_vec();
                let mut out = last_leaf_reference(a, contents, state);
                for alt in alts {
                    out = out.max(last_leaf_reference(a, alt, state));
                }
                out
            }
            DocNode::WithContext { doc, .. } => last_leaf_reference(a, doc, state),
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                ..
            } => last_leaf_reference(a, break_doc, state)
                .max(last_leaf_reference(a, flat_doc, state)),
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let kids: Vec<DocId> = range.resolve(&a.borrow_children()).to_vec();
                kids.into_iter()
                    .fold(state, |state, kid| last_leaf_reference(a, kid, state))
            }
            _ => state,
        }
    }

    fn render(a: &DocArena, doc: DocId) -> String {
        arena_print_doc(a, doc, &EmbedContext::default())
    }

    /// Every seed of the family, at every depth up to `max_depth`, as `(dag, tree)` — once
    /// with every walk memoized from its first visit, and once at the production threshold
    /// ([`WALK_MEMO_AFTER`]) over docs deep enough to cross it, so a memo that starts
    /// mid-walk is graded too: entries computed before it started, recorded after.
    fn for_each_pair(max_depth: u32, mut check: impl FnMut(&DocArena, DocId, DocId, &str)) {
        for (after, max_depth) in [(0, max_depth), (WALK_MEMO_AFTER, max_depth + 3)] {
            test_memo_after::with(after, || {
                for seed in 0..300 {
                    for depth in 1..=max_depth {
                        let a = DocArena::new();
                        let dag = build(&a, seed, 0, depth, true);
                        let tree = build(&a, seed, 0, depth, false);
                        let case = format!("memo after {after}, seed {seed}, depth {depth}");
                        check(&a, dag, tree, &case);
                    }
                }
            });
        }
    }

    #[test]
    fn holds_line_suffix_answers_a_shared_subtree_as_its_copies() {
        for_each_pair(6, |a, dag, tree, case| {
            assert_eq!(
                a.holds_line_suffix(dag),
                a.holds_line_suffix(tree),
                "{case}"
            );
        });
    }

    #[test]
    fn can_break_answers_a_shared_subtree_as_its_copies() {
        for_each_pair(6, |a, dag, tree, case| {
            assert_eq!(a.can_break(dag), a.can_break(tree), "{case}");
        });
    }

    #[test]
    fn pending_run_at_end_answers_a_shared_subtree_as_its_copies() {
        let mut pending = 0;
        for_each_pair(6, |a, dag, tree, case| {
            let run = a.pending_run_at_end(dag, "");
            pending += usize::from(run.is_some());
            assert_eq!(run, a.pending_run_at_end(tree, ""), "{case}");
        });
        assert!(
            pending > 0,
            "the family leaves no run pending: it grades nothing"
        );
    }

    /// Alternatives that each queue a block of a different width ahead of a shared
    /// subtree reach it in one state per subset sum of those widths: `x = if_break(
    /// suffix(/*…*/) x, x)`, nested, the `i`th block `2^i` wide. The saturated width keeps
    /// the states per node within [`RUN_WIDTH_CAP`]'s, and the answer is the copies'.
    #[test]
    fn pending_run_at_end_states_per_node_stay_within_the_width_cap() {
        let nest = |a: &DocArena, depth: u32, shared: bool| {
            fn level(a: &DocArena, i: u32, shared: bool) -> DocId {
                if i == 0 {
                    return a.text("a");
                }
                let x = level(a, i - 1, shared);
                let y = if shared { x } else { level(a, i - 1, shared) };
                let block = a.text_pooled(&format!("/*{}*/", "x".repeat(1 << (i - 1))));
                a.if_break(a.concat(&[a.line_suffix(block), x]), y)
            }
            level(a, depth, shared)
        };
        let depth = 16;
        let a = DocArena::new();
        let dag = nest(&a, depth, true);
        let nodes = a.nodes.borrow().len();
        let tree = nest(&a, depth, false);
        assert_eq!(
            a.pending_run_at_end(dag, ""),
            a.pending_run_at_end(tree, "")
        );
        let (nodes_ref, children, pool) =
            (a.nodes.borrow(), a.children.borrow(), a.text_pool.borrow());
        let store = WalkStore {
            nodes: &nodes_ref,
            children: &children,
            pool: &pool,
            source: "",
        };
        let mut memo = WalkMemo::starting_after(0);
        store.suffix_walk(dag, SuffixWalk::default(), &mut memo);
        let entries = memo.map.map_or(0, |m| m.len());
        assert!(
            entries <= nodes * (usize::from(RUN_WIDTH_CAP) + 1),
            "{entries} memo entries over {nodes} nodes"
        );
    }

    /// The saturated width answers every fits question the true width does, for any
    /// remaining width up to the print width, in either mode, breaking run or not.
    #[test]
    fn a_run_wider_than_the_cap_fits_as_the_cap() {
        let a = DocArena::new();
        let run = |width: u16, has_line_comment: bool| {
            let end = a.embed_end(
                Some(PendingRun {
                    width,
                    has_line_comment,
                    ends_in_line_comment: has_line_comment,
                    certain: true,
                }),
                false,
            );
            a.concat(&[a.line_suffix(a.text("/* c */")), end.doc(), a.text("tail")])
        };
        for has_line_comment in [false, true] {
            let capped = run(RUN_WIDTH_CAP, has_line_comment);
            for wider in [RUN_WIDTH_CAP + 1, RUN_WIDTH_CAP + 50, u16::MAX] {
                let true_width = run(wider, has_line_comment);
                for width in 0..=PRINT_WIDTH {
                    for mode in [Mode::Flat, Mode::Break] {
                        assert_eq!(
                            arena_fits(&a, capped, width, mode),
                            arena_fits(&a, true_width, width, mode),
                            "{wider} columns, {width} remaining, {mode:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn may_end_on_comment_text_answers_a_shared_subtree_as_its_copies_and_as_the_threaded_walk() {
        let mut comments = 0;
        for_each_pair(6, |a, dag, tree, case| {
            let answer = a.may_end_on_comment_text(dag, "");
            comments += usize::from(answer);
            assert_eq!(answer, a.may_end_on_comment_text(tree, ""), "{case}");
            let reference = last_leaf_reference(a, tree, LastLeaf::Nothing) == LastLeaf::Comment;
            assert_eq!(answer, reference, "{case}");
        });
        assert!(
            comments > 0,
            "the family never ends on a comment: it grades nothing"
        );
    }

    #[test]
    fn flattening_a_shared_subtree_prints_as_flattening_its_copies() {
        for_each_pair(5, |a, dag, tree, case| {
            assert_eq!(
                render(a, a.remove_lines(dag)),
                render(a, a.remove_lines(tree)),
                "remove_lines, {case}"
            );
            assert_eq!(
                render(a, a.atomize(dag)),
                render(a, a.atomize(tree)),
                "atomize, {case}"
            );
        });
    }

    #[test]
    fn flattening_a_shared_subtree_shares_its_copy() {
        let a = DocArena::new();
        let shared = a.group(a.concat(&[a.text("a"), a.line(), a.text("b")]));
        let doc = a.if_break(
            a.concat(&[shared, a.text(","), shared]),
            a.concat(&[shared, shared]),
        );
        let flat = a.remove_lines(doc);
        let DocNode::Concat(range) = a.get(flat).clone() else {
            panic!("the flat arm of an `if_break` flattens to its concat");
        };
        let kids = range.resolve(&a.borrow_children()).to_vec();
        assert_eq!(kids.len(), 2);
        assert_eq!(kids[0], kids[1], "one flattened copy, named twice");
        assert_eq!(render(&a, flat), "a ba b");
    }

    /// Every node and child list `a` holds, in allocation order — what two arenas built
    /// alike must agree on when two walks are to have built the same copy.
    fn arena_dump(a: &DocArena) -> String {
        format!("{:?}\n{:?}", a.nodes.borrow(), a.children.borrow())
    }

    /// The flattening loop ([`DocArena::flatten_lines_loop`]) builds the recursion's copy:
    /// on two arenas built alike, a walk entering the loop at once (no frames), one
    /// switching to it mid-walk, and the recursion return the same node and allocate the
    /// same nodes and child lists, in the same order, in both modes — the DAG and the tree form, with the memo answering from
    /// the first visit and from the production threshold.
    #[test]
    fn flattening_in_the_loop_allocates_the_recursions_copy() {
        for (after, max_depth) in [(0, 5), (WALK_MEMO_AFTER, 8)] {
            test_memo_after::with(after, || {
                for seed in 0..300 {
                    for depth in 1..=max_depth {
                        for shared in [true, false] {
                            for mode in [FlattenMode::RemoveLines, FlattenMode::Atomize] {
                                let copy = |frames: u32| {
                                    let a = DocArena::new();
                                    let doc = build(&a, seed, 0, depth, shared);
                                    let flat = a.flatten_lines_impl(
                                        doc,
                                        mode,
                                        &mut WalkMemo::new(),
                                        frames,
                                    );
                                    (flat, arena_dump(&a), render(&a, flat))
                                };
                                let recursion = copy(FLATTEN_FRAMES);
                                for frames in [0, 1, 2] {
                                    assert_eq!(
                                        copy(frames),
                                        recursion,
                                        "{mode:?}, {frames} frames, memo after {after}, \
                                         seed {seed}, depth {depth}, shared {shared}"
                                    );
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    /// The same over a doc holding every node kind the walk copies — the kinds the
    /// generated family leaves out among them (a fill, an align, a dedent, a context, a
    /// gated state, an `indent_if_break`, a multi-line text, a break marker), each over a
    /// shared subtree.
    #[test]
    fn flattening_in_the_loop_copies_every_kind_as_the_recursion_does() {
        let copy = |mode: FlattenMode, frames: u32| {
            let a = DocArena::new();
            let shared = a.group(a.concat(&[a.text("a"), a.line(), a.text("b")]));
            let keyed = a.group_with_id(a.concat(&[shared, a.softline()]));
            let kinds = [
                a.fill(&[shared, a.line(), a.text("c"), a.softline(), shared]),
                a.align(2, a.concat(&[a.hardline(), shared])),
                a.align_root(0, shared),
                a.dedent(a.concat(&[shared, a.literalline()])),
                a.with_context(shared, super::DocContext::default()),
                a.conditional_group(&[
                    shared,
                    a.gated_state(a.concat(&[shared, a.line()]), shared),
                ]),
                keyed.doc(),
                a.indent_if_break(shared, keyed),
                a.line_suffix(a.concat(&[a.text(" "), shared])),
                a.multiline_text("/* a\n b */"),
                a.concat(&[a.break_parent(), a.flush_break(), a.line_suffix_boundary()]),
                a.if_break(a.text("x"), shared),
                // a broken group holding states, which `remove_lines` keeps as they are:
                // no constructor builds one, so the walk must not visit them either
                a.alloc(DocNode::Group {
                    contents: shared,
                    expanded_states: a.alloc_children(&[a.concat(&[shared, a.line()])]),
                    keyed: false,
                    should_break: true,
                }),
            ];
            let doc = a.group_break(a.concat(&kinds));
            let flat = a.flatten_lines_impl(doc, mode, &mut WalkMemo::new(), frames);
            (flat, arena_dump(&a), render(&a, flat))
        };
        for mode in [FlattenMode::RemoveLines, FlattenMode::Atomize] {
            for frames in [0, 1, 2, 3] {
                assert_eq!(
                    copy(mode, frames),
                    copy(mode, FLATTEN_FRAMES),
                    "{mode:?}, {frames} frames"
                );
            }
        }
    }

    /// The loop counts the recursion's doc-walk visits ([`crate::fanout_probe::walk`]):
    /// one per node entered, a memo hit included, whichever walk enters it.
    #[cfg(feature = "fanout_check")]
    #[test]
    fn flattening_in_the_loop_counts_the_recursions_walks() {
        use crate::fanout_probe::{Budget, arm, disarm};
        let walks = |mode: FlattenMode, frames: u32| {
            let a = DocArena::new();
            let shared = a.group(a.concat(&[a.text("a"), a.line(), a.text("b")]));
            let mut doc = a.concat(&[shared, a.softline(), shared]);
            for _ in 0..4 {
                doc = a.indent(a.concat(&[doc, a.line(), shared]));
            }
            arm(Budget {
                builds: u64::MAX,
                walks: u64::MAX,
                scans: u64::MAX,
                predicates: u64::MAX,
                nodes: u64::MAX,
            });
            a.flatten_lines_impl(doc, mode, &mut WalkMemo::new(), frames);
            disarm().walks
        };
        for mode in [FlattenMode::RemoveLines, FlattenMode::Atomize] {
            let recursion = walks(mode, FLATTEN_FRAMES);
            assert!(recursion > 0, "{mode:?}: the walk counted its visits");
            for frames in [0, 1, 2, 3] {
                assert_eq!(walks(mode, frames), recursion, "{mode:?}, {frames} frames");
            }
        }
    }

    /// A subtree shared in the input stays shared in the copy below the frames the
    /// recursion spends, where the loop walks it: one copy, named twice.
    #[test]
    fn flattening_past_the_recursion_shares_a_shared_subtrees_copy() {
        let a = DocArena::new();
        let shared = a.group(a.concat(&[a.text("a"), a.line(), a.text("b")]));
        let mut doc = a.if_break(
            a.concat(&[shared, a.text(","), shared]),
            a.concat(&[shared, shared]),
        );
        for _ in 0..FLATTEN_FRAMES * 2 {
            doc = a.indent(doc);
        }
        for mode in [FlattenMode::RemoveLines, FlattenMode::Atomize] {
            let mut flat = a.flatten_lines_impl(doc, mode, &mut WalkMemo::new(), FLATTEN_FRAMES);
            for _ in 0..FLATTEN_FRAMES * 2 {
                let DocNode::Indent(inner) = a.get(flat).clone() else {
                    panic!("an indent flattens to an indent");
                };
                flat = inner;
            }
            let DocNode::Concat(range) = a.get(flat).clone() else {
                panic!("the flat arm of an `if_break` flattens to its concat");
            };
            let kids = range.resolve(&a.borrow_children()).to_vec();
            assert_eq!(kids.len(), 2);
            assert_eq!(
                kids[0], kids[1],
                "{mode:?}: one flattened copy, named twice"
            );
            assert_eq!(render(&a, flat), "a ba b", "{mode:?}");
        }
    }

    /// A doc nested far deeper than [`FLATTEN_FRAMES`] flattens on a small stack: past
    /// them its levels cost heap, not a frame each. The copy keeps every text and no line.
    #[test]
    fn a_deep_doc_flattens_without_a_frame_per_level() {
        const LEVELS: usize = 100_000;
        let leaves = std::thread::Builder::new()
            .stack_size(256 << 10)
            .spawn(|| {
                let a = DocArena::new();
                let mut doc = a.text("a");
                for _ in 0..LEVELS {
                    doc = a.group(a.concat(&[a.text("!"), a.line(), doc]));
                }
                [a.remove_lines(doc), a.atomize(doc)].map(|flat| {
                    // the copy's leaves, by kind, walked without recursing
                    let (mut texts, mut lines) = (0, 0);
                    let mut stack = vec![flat];
                    while let Some(id) = stack.pop() {
                        match a.get(id).clone() {
                            DocNode::Text(_) => texts += 1,
                            DocNode::Line(_) => lines += 1,
                            DocNode::Group { contents, .. } => stack.push(contents),
                            DocNode::Concat(range) => {
                                stack.extend_from_slice(range.resolve(&a.borrow_children()));
                            }
                            other => panic!("a group, a concat or a leaf, not {other:?}"),
                        }
                    }
                    (texts, lines)
                })
            })
            .expect("spawn")
            .join()
            .expect("the flatten ran");
        // each level's `line` is a space in the copy, a text of its own
        assert_eq!(leaves, [(LEVELS * 2 + 1, 0); 2]);
    }
}
