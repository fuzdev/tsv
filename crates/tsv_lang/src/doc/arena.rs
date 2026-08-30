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

use smallvec::SmallVec;

use crate::hash::FxHashMap;

use crate::Span;
use crate::config::TAB_WIDTH;
use crate::printing::{next_lf, visual_width};

#[cfg(feature = "comment_check")]
use crate::comment_ledger::{DocumentKey, comment_check_enabled, document_key};

use super::DocBuf;
#[cfg(feature = "swallow_check")]
use super::swallow::swallow_check_enabled;
use super::types::{
    CachedWidth, DocContext, DocText, GroupId, LineKind, Mode, PoolSpan, TEXT_WIDTH_HAS_NEWLINE,
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
    /// Used for indentable (JSDoc / `*`-aligned) multi-line block comments,
    /// whose continuation lines all use the uniform hardline (context-indent)
    /// layout. Always contains a newline, so it forces enclosing groups to break
    /// (`will_break` is true) exactly like the hardlines it replaces.
    MultilineText { span: PoolSpan, first_width: u16 },

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
    Group {
        contents: DocId,
        expanded_states: ChildRange,
        id: Option<GroupId>,
        should_break: bool,
    },

    /// Conditional rendering based on whether a group breaks.
    ///
    /// `group_id == None` keys on the immediately enclosing group (the common
    /// case). `group_id == Some(id)` keys on a specific group's resolved mode
    /// (like `IndentIfBreak`), so the conditional can react to a group it is not
    /// nested inside — e.g. a block-tag head's `}` dangling after its head group.
    IfBreak {
        break_doc: DocId,
        flat_doc: DocId,
        group_id: Option<GroupId>,
    },

    /// Conditionally indent based on whether a specific group broke
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

    /// A conditional-group state guarded by a never-fits probe — the
    /// member-chain flat-object hug window.
    ///
    /// Meaningful only as a non-final state of a conditional group. At state
    /// selection the renderer first measures `probe` flat on a hypothetical
    /// fresh line one indent level deeper than the group; if THAT fits, the
    /// state is skipped entirely — the layout the probe stands for (the
    /// expanded member chain keeping its last argument flat on the
    /// continuation line) is the settled form, and admitting this state would
    /// steal it. Only when the probe cannot fit anywhere is `contents`
    /// measured and rendered like any other state. Everywhere else — fits
    /// walks, `will_break`, rebuilds — the node is transparent to `contents`;
    /// `probe` is measured only, never rendered (its nodes are shared with the
    /// group's flat state, so nothing exists only inside it).
    GatedState { probe: DocId, contents: DocId },
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
// pinned per target: 24 B on 64-bit (the native flagship) and 16 B on wasm32 (the shipped WASM
// bundles, where the locality/allocator budget matters most).
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
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<DocNode>() == 24);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(size_of::<DocNode>() == 16);

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
const _: () = assert!(size_of::<ArenaCommand>() == 8);

// `indent_mode` is printed as the two logical values it packs, so the derived
// lint's "field is unused" is exactly wrong here: nothing about the value is hidden.
#[allow(clippy::missing_fields_in_debug)]
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
/// ⚠️ **A format-ignored verbatim slice is layout-opaque, and that is the
/// opposite verdict from every other newline-bearing text.** Its embedded
/// newlines are *source* layout, not a break the enclosing group must honor
/// (prettier's `printIgnored` string is likewise invisible to `willBreak`), so
/// it reports [`LAYOUT_BREAKS_SOFT`] — no single-line width, no forced break —
/// where a line-continuation string reports [`LAYOUT_BREAKS_FORCED`]. The fits
/// walk still sees the newline through the same width slot.
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
/// hottest fill on the board, so the duplicate decode stays — it is one
/// comparison against the ONE sentinel this type has, and the policy that keeps
/// it at one is on [`pooled_text_width`].
#[inline]
fn text_subtree_layout(t: &DocText) -> u32 {
    match t {
        DocText::VerbatimSpan(_, w) => match *w {
            TEXT_WIDTH_HAS_NEWLINE => LAYOUT_BREAKS_SOFT,
            w => u32::from(w),
        },
        // A newline-bearing Text (a line-continuation string) breaks the
        // enclosing group, like `MultilineText` — the width cache flags it via
        // `HasNewline`, and under the eager `pooled_text_width` policy that flag
        // is always set at build, so this reads the answer rather than
        // approximating it.
        _ => match t.cached_width() {
            CachedWidth::Width(w) => u32::from(w),
            CachedWidth::HasNewline => LAYOUT_BREAKS_FORCED,
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

/// Longest slice [`pooled_text_width`] measures with its fused byte walk. Past
/// it, the scan shape flips to the searcher-based one: `contains('\n')` and
/// `is_ascii` are SIMD and the tab count auto-vectorizes (it has no early exit),
/// so on a long slice three vector passes beat one scalar walk — while on a
/// short one their setup, paid regardless of length, is the entire cost. Text
/// nodes are short (a CSS property name, a value chunk), but not uniformly: the
/// TS printer's tail runs long enough that an ungated fused walk measured a real
/// regression on TS while CSS never noticed the gate at all. The crossover is
/// broad and 32 sits in the flat middle of it. Only a *speed* switch — both arms
/// answer identically, and one oracle grades them.
const FUSED_WIDTH_SCAN_MAX: usize = 32;

/// The eager width-cache policy for doc text, and it has **no exceptions**:
/// pool-stored text ([`DocText::Pooled`], `MultilineText` first lines),
/// verbatim source slices ([`DocArena::source_span`], identifier names
/// included), and `text()` statics (amortized through the arena's static cache
/// — measured once per unique string, not per node) **always** cache a real
/// width or the newline sentinel at build. So every width query (the fits
/// walk, `render_text`'s column advance) answers from the node alone, the fits
/// path never borrows the pool, and render's per-text byte scan is skipped.
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
/// measures were unified onto [`fused_ascii_width`]. Re-take a width-policy
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
/// The measured width is clamped below the sentinel. Unlike the `u32`
/// flat-width cache above (where aliasing needs a ~4 GB subtree and is benign
/// anyway), a `u16` alias is reachable — a single-line non-ASCII text ≥65,535
/// columns — and `as u16` alone would be wrong twice over: 65,535 aliases
/// `TEXT_WIDTH_HAS_NEWLINE` (fits would treat the line as ending inside the
/// text) and ≥65,536 wraps (a huge text cached as narrow → "always fits").
/// Clamping is verdict-preserving: every fits comparison is against a print
/// width orders of magnitude below the clamp, so "65,534" and the true width
/// answer identically. The same holds for the other consumer, `render_text`'s
/// column advance — the column only feeds threshold comparisons (print width,
/// `first_line_offset`) far below the clamp, and resets at each newline.
///
/// One forward byte pass decides all three facts the width needs — is there a
/// newline, is the slice ASCII, how many tabs does it hold — because three
/// separate searchers cost more in *setup* (paid regardless of length) than one
/// walk costs in total on the short slice that actually arrives here. Slices
/// past [`FUSED_WIDTH_SCAN_MAX`] take the searcher shape instead.
///
/// Answers identically to probing `contains('\n')` and then
/// [`crate::printing::visual_width`]: on an all-ASCII slice the
/// loop accumulates `1` per byte and `TAB_WIDTH` per tab, which is exactly that
/// function's ASCII fast path, `len + tabs * (TAB_WIDTH - 1)`; a `\n` seen before
/// any non-ASCII byte yields the same sentinel the `contains` probe would have;
/// and the first non-ASCII byte hands the **whole** slice to the searcher arm, so
/// a newline sitting *after* that byte is still found.
///
/// ⚠️ It mirrors that **ASCII fast path**, where a control character counts as
/// one column — deliberately *not* `printing::ascii_char_width`, which counts it
/// as zero and which only the grapheme-walking path uses (see
/// `visual_width_mixed`). The two disagree on purpose; a fused walk that reached
/// for the "obvious" shared helper would silently change every width holding a
/// control byte. The exhaustive equivalence test grades this arm with `\x00`,
/// `\x1b` and `\x7f` precisely because no corpus does.
#[inline]
fn pooled_text_width(s: &str) -> u16 {
    match fused_ascii_width(s) {
        FusedWidth::Width(w) => w.min(TEXT_WIDTH_HAS_NEWLINE as usize - 1) as u16,
        FusedWidth::Newline => TEXT_WIDTH_HAS_NEWLINE,
        FusedWidth::Searcher => pooled_text_width_scanned(s),
    }
}

/// [`fused_ascii_width`]'s verdict for one slice.
pub(super) enum FusedWidth {
    /// Single-line, all-ASCII: the slice's visual width, unclamped.
    Width(usize),
    /// A `\n` was reached before any non-ASCII byte, so the slice has no
    /// single-line width.
    Newline,
    /// The walk declined: the slice is past [`FUSED_WIDTH_SCAN_MAX`], or it holds a
    /// non-ASCII byte. Either way the caller measures the **whole** slice with the
    /// searcher shape — a grapheme cluster can start on an ASCII byte the walk
    /// already counted, so the accumulated width is discarded, not resumed.
    Searcher,
}

/// The one fused width walk, behind [`pooled_text_width`] — the single
/// build-time measure every doc text goes through under the eager policy above.
/// It was shared with a second, on-demand measure in the fits path
/// (`arena_fits`'s `text_flat_width`) until that policy lost its last exception
/// and the on-demand arm lost its producer; the two had previously spelled the
/// question different ways — the fits arm as `contains('\n')` then
/// [`crate::printing::visual_width`], two searcher-driven passes over a slice
/// whose median length is a handful of bytes — and only this one had the
/// exhaustive oracle (`pooled_text_width_tests`). One walk, one oracle, one
/// answer.
///
/// See [`pooled_text_width`]'s doc for why one pass beats three searchers on a
/// short slice, why the ASCII arm counts a control byte as one column (mirroring
/// `visual_width`'s ASCII fast path, **not** `printing::ascii_char_width`), and
/// why the non-ASCII handoff re-measures from the start. The length gate is
/// **inside** rather than at each caller: it is part of the answer this function
/// owns (`Searcher` means "the fused walk declines", for either reason), so the
/// two callers cannot drift apart on where the crossover sits.
#[inline]
pub(super) fn fused_ascii_width(s: &str) -> FusedWidth {
    if s.len() > FUSED_WIDTH_SCAN_MAX {
        return FusedWidth::Searcher;
    }
    let mut width = 0usize;
    for &b in s.as_bytes() {
        match b {
            b'\n' => return FusedWidth::Newline,
            b'\t' => width += TAB_WIDTH,
            0x00..=0x7f => width += 1,
            _ => return FusedWidth::Searcher,
        }
    }
    FusedWidth::Width(width)
}

/// The searcher-based arm of [`pooled_text_width`]: the whole-slice shape, for a
/// slice too long for the fused walk or holding a non-ASCII byte. Outlined to
/// keep that walk lean and inlinable, mirroring the split in
/// `arena_render::update_pos_for_text` — but, unlike that one's helper,
/// **not `#[cold]`**: a long slice is a normal input here, not a rare one (the TS
/// printer's text nodes run past the gate often enough that marking this arm cold
/// would mispredict against the corpus that needs it most).
///
/// Takes the whole slice, not the scanned remainder — a grapheme cluster can
/// start on the ASCII byte *before* the first non-ASCII one, so only measuring
/// from the beginning is cluster-correct.
#[inline(never)]
fn pooled_text_width_scanned(s: &str) -> u16 {
    if s.contains('\n') {
        TEXT_WIDTH_HAS_NEWLINE
    } else {
        visual_width(s, TAB_WIDTH).min(TEXT_WIDTH_HAS_NEWLINE as usize - 1) as u16
    }
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
    /// Parked whole-source line-break table backing the per-file
    /// `build_line_breaks` in each `format_in` — taken (moved out), filled,
    /// and parked back cleared with capacity retained, like `render_scratch`.
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
    /// Parked node-keyed doc-share map: an AST-node pointer plus a consumer-owned
    /// build tag (`(usize, u8)`) → the `DocId` already built for it. The tag is
    /// what lets one node hold **several** entries — the same argument built
    /// under different printer state, or by a different builder — so the
    /// consumer's "a hit is byte-identical to a rebuild" rule is carried by the
    /// key rather than by refusing to cache. Storage for the TS printer's
    /// member-chain argument sharing, parked here — on the reused per-thread
    /// arena — so the table's capacity warms once instead of a fresh `HashMap`
    /// resize chain per printer/file. The consumer owns the protocol: it clears
    /// the map at every share-scope entry AND exit, so between scopes it is
    /// logically empty (stale `DocId`s from a prior document are unreachable —
    /// cleared before any read) and only capacity persists across `reset()`. Only
    /// ever affects allocation, never output (a hit is byte-identical to a rebuild
    /// by construction of the consumer's key). Hashed by [`crate::hash::FxHasher`]
    /// rather than SipHash — the consumer only ever does `get`/`insert`/`clear`,
    /// never iterates, so the hasher is unobservable (see `hash`'s module docs).
    share_map_scratch: RefCell<FxHashMap<(usize, u8), DocId>>,
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
    /// probes never alias the `RefCell` stores. Inline by design: the arena
    /// lives on the stack or in a thread-local and is only ever borrowed,
    /// never moved after construction, so the array adds no per-use
    /// indirection.
    static_cache: [Cell<StaticSlot>; STATIC_CACHE_SLOTS],
    /// The current document's format generation, keying the validity of the
    /// interned node halves in `static_cache` and the singleton cells
    /// (`empty_node`, `line_nodes`, `line_suffix_boundary_node`,
    /// `break_parent_node`, `flush_break_node`). Starts
    /// at 1 (0 marks a never-stamped slot) and is bumped by `reset()`, so a
    /// prior document's `node_id`s — invalidated by the reset — can never be
    /// returned for the new document.
    format_gen: Cell<u32>,
    /// The interned [`Self::empty`] node for the current document (generation,
    /// id) — `empty()` is the single hottest static (~1/3 of static allocs), so
    /// it gets a dedicated slot with no hash probe. Valid iff the generation
    /// matches `format_gen`.
    empty_node: Cell<(u32, DocId)>,
    /// The interned [`DocNode::Line`] node per [`LineKind`] for the current
    /// document (generation, id), direct-indexed by the kind's discriminant —
    /// no hash probe, like `empty_node`. A `Line` node carries no per-use
    /// state (mode and indent are supplied per visit by the enclosing render
    /// command), so every `line()`/`softline()`/`hardline()`/`literalline()`
    /// in a document can return one shared node — the layout analog of
    /// "statics are position-free". Valid iff the generation matches
    /// `format_gen`.
    line_nodes: [Cell<(u32, DocId)>; 4],
    /// The interned [`DocNode::LineSuffixBoundary`] node for the current
    /// document (generation, id) — stateless like `Line`, same dedicated-cell
    /// interning. Valid iff the generation matches `format_gen`.
    line_suffix_boundary_node: Cell<(u32, DocId)>,
    /// The interned [`DocNode::BreakParent`] node for the current document
    /// (generation, id) — stateless like `Line`, same dedicated-cell
    /// interning. Valid iff the generation matches `format_gen`.
    break_parent_node: Cell<(u32, DocId)>,
    /// The interned [`DocNode::FlushBreak`] node for the current document
    /// (generation, id) — stateless like `Line`, same dedicated-cell
    /// interning. Valid iff the generation matches `format_gen`.
    flush_break_node: Cell<(u32, DocId)>,
    /// The interned [`DocNode::FlowProbeEnd`] node for the current document
    /// (generation, id) — stateless like `Line`, same dedicated-cell
    /// interning. Valid iff the generation matches `format_gen`.
    flow_probe_end_node: Cell<(u32, DocId)>,
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
#[derive(Clone, Copy, Debug)]
struct StaticSlot {
    ptr: usize,
    len: u32,
    width: u16,
    /// Format generation that stamped `node_id`; 0 = never stamped.
    node_gen: u32,
    /// The interned node for the generation in `node_gen`.
    node_id: DocId,
}

impl StaticSlot {
    /// An empty slot: `ptr == 0` is never a real entry (references are never
    /// null — even `""` has a non-null dangling address).
    const EMPTY: Self = Self {
        ptr: 0,
        len: 0,
        width: 0,
        node_gen: 0,
        node_id: DocId(0),
    };
}

/// 2048 slots (× 24 B on 64-bit = 48 KB inline). Kept in lockstep with the
/// slot-hash shift — see the assert below.
///
/// **Sized against the collision draw, not against the population.** The
/// unique-static population is ~165–190 across real corpora and the *per
/// document* working set is ~55, so a 512-slot table looks like ten times the
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
/// the tail; 4096 is worth a further −0.01% and is not worth 96 KB. 48 KB sits
/// on a 32 MiB format-worker stack (`cli::stack::STACK_SIZE`) and in one
/// per-thread arena per binding, never per file.
const STATIC_CACHE_SLOTS: usize = 2048;

// The slot index is the TOP 11 BITS of the 64-bit multiplicative hash
// (`>> 53` in `text`), which is provably `< 2048` — that both elides
// the array bounds check and hard-couples the shift to the slot count. This
// assert makes changing one without the other a compile error.
const _: () = assert!(STATIC_CACHE_SLOTS == 1 << 11);

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
    // `large_stack_arrays`: the 48 KB `static_cache` is inline **by design** —
    // see [`STATIC_CACHE_SLOTS`] for why it is that size and the field doc for
    // why it is not behind a pointer (the hot path indexes it off `&self` with
    // no dependent load). Both constructors are per-thread, never per file, and
    // the array is `[const { … }; N]`, so the initializer is materialized in
    // place rather than copied from a temporary. Consumers that park the arena
    // between calls already box it (`tsv_arena::with_doc_arena`).
    #[allow(clippy::large_stack_arrays)]
    pub fn new() -> Self {
        Self {
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
            share_map_scratch: RefCell::new(FxHashMap::default()),
            layout_cache: RefCell::new(Vec::new()),
            static_cache: [const { Cell::new(StaticSlot::EMPTY) }; STATIC_CACHE_SLOTS],
            format_gen: Cell::new(1),
            empty_node: Cell::new((0, DocId(0))),
            line_nodes: [const { Cell::new((0, DocId(0))) }; 4],
            line_suffix_boundary_node: Cell::new((0, DocId(0))),
            break_parent_node: Cell::new((0, DocId(0))),
            flush_break_node: Cell::new((0, DocId(0))),
            flow_probe_end_node: Cell::new((0, DocId(0))),
            flow_probe: RefCell::new(FlowProbeState::default()),
            flow_probe_broke: Cell::new(false),
            #[cfg(debug_assertions)]
            flow_probe_fresh: Cell::new(false),
            #[cfg(feature = "swallow_check")]
            line_comment_ids: RefCell::new(Vec::new()),
            #[cfg(feature = "comment_check")]
            comment_docs: RefCell::new(Vec::new()),
        }
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
    // `large_stack_arrays`: see [`Self::new`].
    #[allow(clippy::large_stack_arrays)]
    pub fn with_source_size_hint(source_len: usize) -> Self {
        let estimated_nodes = source_len * 2;
        let estimated_children = estimated_nodes / 2;
        Self {
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
            share_map_scratch: RefCell::new(FxHashMap::default()),
            // The fitting memos top out at `nodes.len()` (~= `estimated_nodes`),
            // growing from 0 via repeated `resize(nodes.len(), …)`; pre-reserve
            // to absorb those reallocs. Only capacity changes — never values.
            layout_cache: RefCell::new(Vec::with_capacity(estimated_nodes)),
            static_cache: [const { Cell::new(StaticSlot::EMPTY) }; STATIC_CACHE_SLOTS],
            format_gen: Cell::new(1),
            empty_node: Cell::new((0, DocId(0))),
            line_nodes: [const { Cell::new((0, DocId(0))) }; 4],
            line_suffix_boundary_node: Cell::new((0, DocId(0))),
            break_parent_node: Cell::new((0, DocId(0))),
            flush_break_node: Cell::new((0, DocId(0))),
            flow_probe_end_node: Cell::new((0, DocId(0))),
            flow_probe: RefCell::new(FlowProbeState::default()),
            flow_probe_broke: Cell::new(false),
            #[cfg(debug_assertions)]
            flow_probe_fresh: Cell::new(false),
            #[cfg(feature = "swallow_check")]
            line_comment_ids: RefCell::new(Vec::new()),
            #[cfg(feature = "comment_check")]
            comment_docs: RefCell::new(Vec::new()),
        }
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
    /// interned *node* halves (and the `empty()`/line/boundary singleton
    /// cells) are invalidated in O(1)
    /// by bumping `format_gen` — their `DocId`s point into the node store this
    /// method just cleared.
    pub fn reset(&mut self) {
        let next = self.format_gen.get().wrapping_add(1);
        if next == 0 {
            // u32 generation wrap (~4.3 B resets in one process): a slot last
            // stamped in the ancient generation with this same value would
            // false-hit and return a dangling id, so hard-clear every node
            // half once per wrap. The width halves stay valid ('static-keyed).
            for slot in &self.static_cache {
                let mut s = slot.get();
                s.node_gen = 0;
                slot.set(s);
            }
            self.empty_node.set((0, DocId(0)));
            for cell in &self.line_nodes {
                cell.set((0, DocId(0)));
            }
            self.line_suffix_boundary_node.set((0, DocId(0)));
            self.break_parent_node.set((0, DocId(0)));
            self.flush_break_node.set((0, DocId(0)));
            self.flow_probe_end_node.set((0, DocId(0)));
            self.format_gen.set(1);
        } else {
            self.format_gen.set(next);
        }
        self.nodes.get_mut().clear();
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
    /// per document.
    ///
    /// Repeated calls with the same static within one format return one
    /// shared node (`text(",")` ×10 K → 1 node): the direct-mapped slot
    /// carries the interned `DocId` alongside the cached width, gated by the
    /// arena's `format_gen` so a `reset()` invalidates every interned node in
    /// O(1). Sharing is output-identical — statics are position-free at
    /// render, nodes are append-only and immutable, and no consumer compares
    /// `DocId` identity (`join_doc` has always shared separator ids). The
    /// width half is amortized the same way as before (measured once per
    /// unique string per arena *lifetime* — the *per-node* eager measure was
    /// a measured loss); fits queries answer from the node alone and
    /// `render_text`'s column advance skips its byte scan.
    ///
    /// Hot path (92–95% of calls on real corpora): the slot hash, one slot
    /// load, and a ptr/len/gen compare. ⚠️ **The hash does not fold.** The
    /// tempting reading is that `s`'s address is a link-time constant, so a
    /// folded call site should carry a literal index — but every shipped target
    /// is PIE, where the address is `image_base + link_offset` and the base is
    /// not known until `execve`. `objdump` shows the arithmetic emitted whole at
    /// every one of the ~860 folded sites (`lea` the RIP-relative address,
    /// `movabs` the constant, `imul`, `shr`), and the consequence is bigger than
    /// four instructions: the slot index is **re-drawn on every exec**, which is
    /// what [`STATIC_CACHE_SLOTS`] is sized against. The miss path — first use
    /// this document, first sighting ever, or collision evict — allocs and
    /// restamps in the cold helper.
    #[inline]
    pub fn text(&self, s: &'static str) -> DocId {
        let ptr = s.as_ptr() as usize;
        // Hash in u64: usize is 32-bit on wasm32, where the Fibonacci constant
        // and the top-11-bit shift would overflow. The `>> 53` keeps the top 11
        // bits ⇒ index < 2048, locked to `STATIC_CACHE_SLOTS` by the assert at
        // its definition (and eliding the bounds check below).
        let slot_i = ((ptr as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 53) as usize;
        let slot = self.static_cache[slot_i].get();
        if slot.ptr == ptr && slot.len as usize == s.len() && slot.node_gen == self.format_gen.get()
        {
            return slot.node_id;
        }
        self.text_miss(s, slot_i, slot)
    }

    /// The cold half of [`Self::text`]: alloc the node and (re)stamp the slot.
    ///
    /// Reuses the slot's cached width when only the node half is stale (the
    /// once-per-static-per-document case); measures it on a true width miss
    /// (first sighting or collision evict).
    #[cold]
    #[inline(never)]
    fn text_miss(&self, s: &'static str, slot_i: usize, slot: StaticSlot) -> DocId {
        let ptr = s.as_ptr() as usize;
        let width = if slot.ptr == ptr && slot.len as usize == s.len() {
            slot.width
        } else {
            pooled_text_width(s)
        };
        let node_id = self.alloc(DocNode::Text(DocText::Static(s, width)));
        self.static_cache[slot_i].set(StaticSlot {
            ptr,
            len: s.len() as u32,
            width,
            node_gen: self.format_gen.get(),
            node_id,
        });
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
        let first = &s[..next_lf(s.as_bytes(), 0)];
        let first_width =
            visual_width(first, TAB_WIDTH).min(TEXT_WIDTH_HAS_NEWLINE as usize - 1) as u16;
        let span = self.pool_push(s);
        self.alloc(DocNode::MultilineText { span, first_width })
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
    /// ⚠️ At its current call-site count LLVM **declines** this `#[inline]` and
    /// emits it as a real symbol (2.3% self on a fuz_app board), so a name
    /// emission — ~15% of all doc nodes — pays a call. Two shapes buy that back
    /// and neither is taken: `#[inline(always)]` here recovers `instructions:u`
    /// −1.20% → −1.51% for **+7,264 B** of `.text` (perf's own inline threshold
    /// is a U-curve this codebase already sits at the minimum of, so pushing it
    /// up is a cycles risk), and a second entry point with a **duplicated** body
    /// reaches −1.45% for +336 B but splits one width computation across two
    /// sites that nothing keeps in agreement. A profile-guided build makes this
    /// choice per call site without either cost.
    #[inline]
    pub fn source_span(&self, span: Span, source: &str) -> DocId {
        let w = pooled_text_width(span.extract(source));
        self.alloc(DocNode::Text(DocText::SourceSpan(span, w)))
    }

    /// [`Self::source_span`] for a **format-ignored verbatim slice** (the
    /// `prettier-ignore` freeze): emits [`DocText::VerbatimSpan`] — identical
    /// in measurement and render, but opaque to `will_break` (full rationale on
    /// the variant's doc). Use ONLY for ignore-directive slices; genuine
    /// multi-line content (line-continuation strings, `<pre>` text) keeps
    /// [`Self::source_span`] so it force-breaks.
    #[inline]
    pub fn verbatim_source_span(&self, span: Span, source: &str) -> DocId {
        let w = pooled_text_width(span.extract(source));
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

    /// Return the per-document interned node held in `cell`, allocating it on
    /// first use within the current document.
    ///
    /// The shared engine behind the singleton builders — [`Self::empty`],
    /// [`Self::line`] and its kind siblings, [`Self::line_suffix_boundary`],
    /// [`Self::break_parent`], and [`Self::flush_break`]: each is a node with no per-use state, so
    /// one node per document serves every call site. Hot path: one cell load
    /// plus a generation compare — no hash, cheaper than even the static
    /// cache's slot probe. `reset()` invalidates every cell in O(1) via the
    /// `format_gen` bump (plus the once-per-u32-wrap hard-clear). The node is
    /// built behind a closure, NOT passed by value: a by-value `DocNode`
    /// argument measured a consistent +0.26..+0.30% instructions (the
    /// aggregate is materialized on the hot path; LLVM does not reliably sink
    /// it into the cold branch), while the closure defers construction into
    /// the per-call-site miss instantiation — hot-path codegen identical to a
    /// hand-specialized pair.
    #[inline]
    fn interned_singleton(
        &self,
        cell: &Cell<(u32, DocId)>,
        make: impl FnOnce() -> DocNode,
    ) -> DocId {
        let (node_gen, node_id) = cell.get();
        if node_gen == self.format_gen.get() {
            return node_id;
        }
        self.interned_singleton_miss(cell, make)
    }

    /// The cold half of [`Self::interned_singleton`]: alloc this document's
    /// node and stamp the cell (once per cell per document). Monomorphized
    /// per call site (one cold body per singleton kind — the same set of cold
    /// fns the hand-specialized form had, written once).
    #[cold]
    #[inline(never)]
    fn interned_singleton_miss(
        &self,
        cell: &Cell<(u32, DocId)>,
        make: impl FnOnce() -> DocNode,
    ) -> DocId {
        let node_id = self.alloc(make());
        cell.set((self.format_gen.get(), node_id));
        node_id
    }

    /// Create an empty doc that produces no output, interned per document.
    ///
    /// `empty()` is the single hottest static text (~1/3 of static allocs on
    /// real corpora), so it interns through a dedicated generation-gated cell
    /// — no hash probe — allocating once per document.
    #[inline]
    pub fn empty(&self) -> DocId {
        self.interned_singleton(&self.empty_node, || DocNode::Text(DocText::Static("", 0)))
    }

    /// Create a normal line break (space if fits, newline if doesn't),
    /// interned per document.
    #[inline]
    pub fn line(&self) -> DocId {
        self.line_node(LineKind::Normal)
    }

    /// Create a soft line that disappears in flat mode, interned per document.
    #[inline]
    pub fn softline(&self) -> DocId {
        self.line_node(LineKind::Soft)
    }

    /// Create a hard line break (always breaks), interned per document.
    #[inline]
    pub fn hardline(&self) -> DocId {
        self.line_node(LineKind::Hard)
    }

    /// Create a literal line break (just newline, no indentation), interned
    /// per document.
    #[inline]
    pub fn literalline(&self) -> DocId {
        self.line_node(LineKind::Literal)
    }

    /// Shared interning path for the four [`LineKind`]s: a `Line` node
    /// carries no per-use state (mode and indent are supplied per visit by
    /// the enclosing render command), so every line of a kind within one
    /// document shares one node — the layout analog of "statics are
    /// position-free". Direct-indexed by the kind's discriminant.
    #[inline]
    fn line_node(&self, kind: LineKind) -> DocId {
        self.interned_singleton(&self.line_nodes[kind as usize], || DocNode::Line(kind))
    }

    //
    // Structural builders
    //

    /// Create a group (try to fit on one line, break all if doesn't fit).
    pub fn group(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::Group {
            contents: doc,
            expanded_states: ChildRange::EMPTY,
            id: None,
            should_break: false,
        })
    }

    /// Create a group that forces break mode during rendering.
    pub fn group_break(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::Group {
            contents: doc,
            expanded_states: ChildRange::EMPTY,
            id: None,
            should_break: true,
        })
    }

    /// Create a group with an ID for tracking whether it broke.
    pub fn group_with_id(&self, doc: DocId, id: GroupId) -> DocId {
        self.alloc(DocNode::Group {
            contents: doc,
            expanded_states: ChildRange::EMPTY,
            id: Some(id),
            should_break: false,
        })
    }

    /// [`Self::group_with_id`] whose break mode is decided by the caller rather than by
    /// fit — prettier's `group(…, { id, shouldBreak })`. The id and the forced break go
    /// together whenever some other doc reads the decision through
    /// [`Self::indent_if_break`]: a caller that emitted [`Self::group_break`] instead would
    /// break the group but leave the reader unable to see it.
    pub fn group_with_id_break(&self, doc: DocId, id: GroupId, should_break: bool) -> DocId {
        self.alloc(DocNode::Group {
            contents: doc,
            expanded_states: ChildRange::EMPTY,
            id: Some(id),
            should_break,
        })
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
            id: None,
            should_break: false,
        })
    }

    /// Create a conditional-group state admitted only while `probe` cannot fit
    /// flat on a fresh line one indent level deeper than the enclosing
    /// conditional group — see [`DocNode::GatedState`].
    pub fn gated_state(&self, probe: DocId, contents: DocId) -> DocId {
        self.alloc(DocNode::GatedState { probe, contents })
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

    /// Conditional rendering based on whether a specific group broke.
    ///
    /// Unlike `if_break`, which keys on the immediately enclosing group, this
    /// keys on `group_id`'s resolved mode — so it can sit outside the group it
    /// reacts to (e.g. a block-tag head's `}` after its head group). During
    /// `fits()` the keyed group is treated as unresolved (flat), so trailing
    /// text after the conditional is still counted toward the group's own break
    /// decision (the `}` stays in the head's width).
    pub fn if_break_with_id(&self, break_doc: DocId, flat_doc: DocId, group_id: GroupId) -> DocId {
        self.alloc(DocNode::IfBreak {
            break_doc,
            flat_doc,
            group_id: Some(group_id),
        })
    }

    /// Conditionally indent based on whether a specific group broke.
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
    pub fn concat(&self, docs: &[DocId]) -> DocId {
        match docs {
            [single] => *single,
            [a, b] => self.concat_pair(*a, *b),
            _ => self.concat_other(docs),
        }
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
    pub fn fill(&self, parts: &[DocId]) -> DocId {
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

    /// Force pending LineSuffix content to be flushed, interned per document
    /// (stateless, like [`Self::line`] — one shared node per document).
    #[inline]
    pub fn line_suffix_boundary(&self) -> DocId {
        self.interned_singleton(&self.line_suffix_boundary_node, || {
            DocNode::LineSuffixBoundary
        })
    }

    /// Force parent group to break, interned per document (stateless, like
    /// [`Self::line`] — one shared node per document).
    #[inline]
    pub fn break_parent(&self) -> DocId {
        self.interned_singleton(&self.break_parent_node, || DocNode::BreakParent)
    }

    /// Flush-scoped break for a deferred trailing run ([`DocNode::FlushBreak`]):
    /// force only the nearest enclosing group with a line opportunity AFTER this
    /// point — where the pending [`Self::line_suffix`] actually flushes — leaving
    /// groups that close before it free to stay flat. Emit it right after the
    /// `line_suffix` it scopes. Interned per document (stateless, like
    /// [`Self::break_parent`]).
    #[inline]
    pub fn flush_break(&self) -> DocId {
        self.interned_singleton(&self.flush_break_node, || DocNode::FlushBreak)
    }

    /// The interned [`DocNode::FlowProbeEnd`] sentinel — pushed only by the render loop
    /// behind a [`DocContext::flow_break_probe`]-flagged subtree, never by a printer.
    pub(super) fn flow_probe_end_node(&self) -> DocId {
        self.interned_singleton(&self.flow_probe_end_node, || DocNode::FlowProbeEnd)
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

    /// Wrap a doc with open and close delimiters.
    #[inline]
    pub fn wrap(&self, open: &'static str, inner: DocId, close: &'static str) -> DocId {
        self.concat(&[self.text(open), inner, self.text(close)])
    }

    /// Wrap a doc in parentheses.
    #[inline]
    pub fn parens(&self, inner: DocId) -> DocId {
        self.wrap("(", inner, ")")
    }

    /// Wrap a doc in square brackets.
    #[inline]
    pub fn brackets(&self, inner: DocId) -> DocId {
        self.wrap("[", inner, "]")
    }

    /// Wrap a doc in curly braces.
    #[inline]
    pub fn braces(&self, inner: DocId) -> DocId {
        self.wrap("{", inner, "}")
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
    /// still the right one.** This function carries ~6–8% self on a real-corpus
    /// board — "once per node" is a small share of *calls* and a large share of
    /// *time* — and `#[cold]` puts it under size-optimized codegen, visibly so:
    /// it spills and immediately reloads four argument registers around the
    /// `Concat` / `Fill` arm's recursion. Dropping the attribute removes exactly
    /// that (the body shrinks by 14 bytes) and still measures `instructions:u`
    /// **+0.02%** — the callers lose more from the un-hinted branch, since the
    /// probe in [`Self::subtree_layout_memo`] is inlined at every call site and
    /// wants this call laid out away from its warm path. Measured; don't re-try
    /// without a new idea.
    ///
    /// ⚠️ **That +0.02% was measured when `Text` still entered here**, and the
    /// probe has since taken `Text` and then the leaf-and-forwarding kinds, so
    /// this function's input mix has lost half its calls and is now 96%
    /// `Concat` — exactly the recursive arm `#[cold]` was chosen for. The
    /// verdict should hold a fortiori, but it is a number about a call mix that
    /// has changed twice, so re-take it rather than quote it.
    #[cold]
    #[inline(never)]
    pub(super) fn subtree_layout_fill(
        id: DocId,
        nodes: &[DocNode],
        children: &[DocId],
        cache: &mut [u32],
    ) -> u32 {
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
            DocNode::LineSuffix(_) | DocNode::LineSuffixBoundary => LAYOUT_BREAKS_SOFT,
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
    pub fn can_break(&self, id: DocId) -> bool {
        let nodes = self.nodes.borrow();
        let children = self.children.borrow();
        Self::can_break_inner(id, &nodes, &children)
    }

    /// The slice-threaded body of [`Self::can_break`] — `pub(super)` so the
    /// `arena_fits` walk's pending-flush veto asks it through the slices the
    /// walk already holds instead of re-borrowing per call (the threading
    /// idiom of [`Self::subtree_layout_fill`]).
    pub(super) fn can_break_inner(id: DocId, nodes: &[DocNode], children: &[DocId]) -> bool {
        match &nodes[id.index()] {
            DocNode::Line(_) => true,
            DocNode::FlowProbeEnd => false,
            DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                Self::can_break_inner(*inner, nodes, children)
            }
            DocNode::AlignRoot { contents, .. } | DocNode::Align { contents, .. } => {
                Self::can_break_inner(*contents, nodes, children)
            }
            DocNode::IndentIfBreak { contents, .. } => {
                Self::can_break_inner(*contents, nodes, children)
            }
            DocNode::Group {
                contents,
                expanded_states,
                ..
            } => {
                if Self::can_break_inner(*contents, nodes, children) {
                    return true;
                }
                if !expanded_states.is_empty() {
                    let kids = expanded_states.resolve(children);
                    if kids
                        .iter()
                        .any(|&kid| Self::can_break_inner(kid, nodes, children))
                    {
                        return true;
                    }
                }
                false
            }
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                ..
            } => {
                Self::can_break_inner(*break_doc, nodes, children)
                    || Self::can_break_inner(*flat_doc, nodes, children)
            }
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let kids = range.resolve(children);
                kids.iter()
                    .any(|&kid| Self::can_break_inner(kid, nodes, children))
            }
            DocNode::WithContext { doc, .. } => Self::can_break_inner(*doc, nodes, children),
            DocNode::LineSuffix(inner) => Self::can_break_inner(*inner, nodes, children),
            // Transparent to contents (the probe is measure-only).
            DocNode::GatedState { contents, .. } => {
                Self::can_break_inner(*contents, nodes, children)
            }
            DocNode::MultilineText { .. } => true,
            // deliberately newline-blind, unlike `subtree_layout_fill`: canBreak asks
            // "is there a breakable `line` in here?", and a Text's embedded newline
            // (line-continuation string, verbatim slice) is content, not a break point
            DocNode::Text(_) | DocNode::LineSuffixBoundary => false,
            DocNode::BreakParent => true,
            // No line of its own; whether a line follows is positional, which a
            // subtree query cannot see.
            DocNode::FlushBreak => false,
        }
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
        self.flatten_lines_impl(id, FlattenMode::RemoveLines)
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
    /// caller, the template-interpolation atomizer, first routes any interpolation
    /// containing a comment or a source newline down a *different* branch, so nothing that
    /// must break can reach here.
    ///
    /// Folding the two into one function under prettier's name is how a multi-line comment
    /// in a flattened arrow signature gets its newline deleted. Two questions, two names.
    pub fn atomize(&self, id: DocId) -> DocId {
        self.flatten_lines_impl(id, FlattenMode::Atomize)
    }

    fn flatten_lines_impl(&self, id: DocId, mode: FlattenMode) -> DocId {
        // Extract node info while borrowing, then release borrow before allocating.
        // This pattern avoids RefCell conflicts since alloc() needs borrow_mut().
        enum Info {
            Keep, // Return id unchanged
            FlattenedMultilineText(String),
            Line(LineKind),
            Indent(DocId),
            Dedent(DocId),
            AlignRoot(usize, DocId),
            Align(u32, DocId),
            Group {
                contents: DocId,
                expanded_states: ChildRange,
                id: Option<GroupId>,
                should_break: bool,
            },
            IfBreakFlat(DocId),
            IndentIfBreakContents(DocId),
            Concat(DocBuf),
            Fill(DocBuf),
            WithContext(DocId, DocContext),
            LineSuffix(DocId),
            BreakParent,
            GatedState(DocId, DocId),
        }

        let info = {
            let nodes = self.nodes.borrow();
            match &nodes[id.index()] {
                DocNode::Text(_) | DocNode::LineSuffixBoundary | DocNode::FlowProbeEnd => {
                    Info::Keep
                }
                // `MultilineText`'s `\n`s are hard lines pre-joined into one body, so it
                // follows `mode` for the same reason a `Line(Hard)` does — see the fn docs.
                DocNode::MultilineText { span, .. } => match mode {
                    FlattenMode::RemoveLines => Info::Keep,
                    FlattenMode::Atomize => {
                        let pool = self.text_pool.borrow();
                        Info::FlattenedMultilineText(span.slice(&pool).replace('\n', ""))
                    }
                },
                DocNode::Line(kind) => Info::Line(*kind),
                DocNode::Indent(inner) => Info::Indent(*inner),
                DocNode::Dedent(inner) => Info::Dedent(*inner),
                DocNode::AlignRoot { n, contents } => Info::AlignRoot(*n, *contents),
                DocNode::Align { n, contents } => Info::Align(*n, *contents),
                DocNode::Group {
                    contents,
                    expanded_states,
                    id: group_id,
                    should_break,
                } => Info::Group {
                    contents: *contents,
                    expanded_states: *expanded_states,
                    id: *group_id,
                    should_break: *should_break,
                },
                DocNode::IfBreak { flat_doc, .. } => Info::IfBreakFlat(*flat_doc),
                DocNode::IndentIfBreak { contents, .. } => Info::IndentIfBreakContents(*contents),
                DocNode::Concat(range) => {
                    let children = self.children.borrow();
                    Info::Concat(DocBuf::from_slice(range.resolve(&children)))
                }
                DocNode::Fill(range) => {
                    let children = self.children.borrow();
                    Info::Fill(DocBuf::from_slice(range.resolve(&children)))
                }
                DocNode::WithContext { doc, context } => Info::WithContext(*doc, context.clone()),
                DocNode::LineSuffix(inner) => Info::LineSuffix(*inner),
                // Both are pure layout-forcing markers with no content: flattening
                // drops them the same way (`Info::BreakParent` → `empty()`).
                DocNode::BreakParent | DocNode::FlushBreak => Info::BreakParent,
                DocNode::GatedState { probe, contents } => Info::GatedState(*probe, *contents),
            }
        }; // nodes borrow dropped here

        let new_id = match info {
            Info::Keep => id,
            Info::FlattenedMultilineText(flat) => self.text_pooled(&flat),
            Info::Line(kind) => match kind {
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
            Info::Indent(inner) => {
                let new_inner = self.flatten_lines_impl(inner, mode);
                self.indent(new_inner)
            }
            Info::Dedent(inner) => {
                let new_inner = self.flatten_lines_impl(inner, mode);
                self.dedent(new_inner)
            }
            Info::AlignRoot(n, contents) => {
                let new_contents = self.flatten_lines_impl(contents, mode);
                self.align_root(n, new_contents)
            }
            Info::Align(n, contents) => {
                let new_contents = self.flatten_lines_impl(contents, mode);
                self.align(n, new_contents)
            }
            Info::Group {
                contents,
                expanded_states,
                id: group_id,
                should_break,
            } => {
                let flat_contents = self.flatten_lines_impl(contents, mode);
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
                        id: group_id,
                        should_break,
                    });
                }
                if should_break {
                    self.alloc(DocNode::Group {
                        contents: flat_contents,
                        expanded_states, // Keep as-is
                        id: group_id,
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
                        let new_kids: DocBuf = kids
                            .into_iter()
                            .map(|kid| self.flatten_lines_impl(kid, mode))
                            .collect();
                        self.alloc_children(&new_kids)
                    };
                    self.alloc(DocNode::Group {
                        contents: flat_contents,
                        expanded_states: flat_states,
                        id: group_id,
                        should_break,
                    })
                }
            }
            Info::IfBreakFlat(flat_doc) => self.flatten_lines_impl(flat_doc, mode),
            Info::IndentIfBreakContents(contents) => self.flatten_lines_impl(contents, mode),
            Info::Concat(kids) => {
                let flattened: DocBuf = kids
                    .into_iter()
                    .map(|kid| self.flatten_lines_impl(kid, mode))
                    .collect();
                self.concat(&flattened)
            }
            Info::Fill(kids) => {
                // Fill becomes regular concat when flattened
                let flattened: DocBuf = kids
                    .into_iter()
                    .map(|kid| self.flatten_lines_impl(kid, mode))
                    .collect();
                self.concat(&flattened)
            }
            Info::WithContext(doc, context) => {
                let new_doc = self.flatten_lines_impl(doc, mode);
                self.with_context(new_doc, context)
            }
            Info::LineSuffix(inner) => {
                let new_inner = self.flatten_lines_impl(inner, mode);
                self.line_suffix(new_inner)
            }
            Info::BreakParent => self.empty(),
            // Rebuild both halves, keeping the gate: under `remove_lines` (which
            // keeps conditional-group states) the state must stay gated; under
            // `atomize` the node is unreachable through a group (states dropped)
            // and the rebuild is inert.
            Info::GatedState(probe, contents) => {
                let new_probe = self.flatten_lines_impl(probe, mode);
                let new_contents = self.flatten_lines_impl(contents, mode);
                self.gated_state(new_probe, new_contents)
            }
        };

        // Rebuilding the tree strands a comment doc's ledger tag on the discarded original
        // (a re-allocated node gets a fresh `DocId`), so the renderer never records the emit
        // and the — verbatim-printed — comment reads as DROPPED. Carry the tag onto each
        // rebuilt node. Every recursion routes through here, so a tagged comment anywhere in
        // the subtree is covered. `comment_check`-only — production output is byte-identical.
        #[cfg(feature = "comment_check")]
        if new_id != id {
            self.retag_comment_doc(new_id, id);
        }

        new_id
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

    /// The parked node-keyed doc-share map (see the field doc), keyed by
    /// `(node pointer, consumer build tag)`. Returned as the
    /// `RefCell` itself — the consumer's share scope spans many interleaved
    /// arena calls, so it borrows point-wise per lookup/insert/clear rather
    /// than holding a `RefMut` open. The consumer owns the clear-at-scope-
    /// entry/exit protocol; this accessor deliberately does NOT clear.
    #[inline]
    pub fn share_map_scratch(&self) -> &RefCell<FxHashMap<(usize, u8), DocId>> {
        &self.share_map_scratch
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
mod pooled_text_width_tests {
    use super::{TEXT_WIDTH_HAS_NEWLINE, pooled_text_width};
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
        if s.contains('\n') {
            TEXT_WIDTH_HAS_NEWLINE
        } else {
            visual_width(s, TAB_WIDTH).min(TEXT_WIDTH_HAS_NEWLINE as usize - 1) as u16
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
        // A single-line text wider than the u16 sentinels must clamp, not alias
        // TEXT_WIDTH_HAS_NEWLINE or wrap.
        for len in [
            TEXT_WIDTH_HAS_NEWLINE as usize - 2,
            TEXT_WIDTH_HAS_NEWLINE as usize - 1,
            TEXT_WIDTH_HAS_NEWLINE as usize,
            TEXT_WIDTH_HAS_NEWLINE as usize + 5,
        ] {
            let ascii = "a".repeat(len);
            assert_agrees(&ascii);
            assert!(pooled_text_width(&ascii) < TEXT_WIDTH_HAS_NEWLINE);
            // Tabs multiply the width, so a far shorter run also clamps.
            let tabs = "\t".repeat(len);
            assert_agrees(&tabs);
        }
    }

    #[test]
    fn agrees_on_long_ascii_runs() {
        // The length range where the replaced shape's SIMD scans were at their
        // best — the fused walk must still agree there.
        for len in [31, 32, 33, 63, 64, 65, 127, 128, 256, 1000] {
            assert_agrees(&"x".repeat(len));
            assert_agrees(&format!("{}\t{}", "x".repeat(len / 2), "y".repeat(len / 2)));
            assert_agrees(&format!("{}\n{}", "x".repeat(len / 2), "y".repeat(len / 2)));
            assert_agrees(&format!("{}é{}", "x".repeat(len / 2), "y".repeat(len / 2)));
        }
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
