//! Arena-based document allocation for efficient Doc tree construction and rendering.
//!
//! Instead of heap-allocating each Doc node individually (Box<Doc>, Vec<Doc>),
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

use crate::Span;
use crate::config::TAB_WIDTH;
use crate::printing::visual_width;

#[cfg(feature = "comment_check")]
use crate::comment_ledger::{DocumentKey, comment_check_enabled, document_key};

use super::DocBuf;
#[cfg(feature = "swallow_check")]
use super::swallow::swallow_check_enabled;
use super::types::{
    DocContext, DocText, GroupId, LineKind, Mode, PoolSpan, TEXT_WIDTH_HAS_NEWLINE,
    TEXT_WIDTH_NOT_COMPUTED,
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
#[derive(Debug, Clone)]
pub enum DocNode {
    /// Text content to output (static, pooled, source-span, or symbol)
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

    /// Set absolute indentation level for nested content
    Align { n: usize, contents: DocId },

    /// Try to fit content on one line; if doesn't fit, break ALL lines in group.
    ///
    /// When `expanded_states` is non-empty, this is a "conditional group" that tries
    /// multiple alternative layouts. `contents` is state[0], expanded_states contains
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
    IndentIfBreak {
        contents: DocId,
        group_id: GroupId,
        negate: bool,
    },

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
}

// `DocNode` must stay free of drop glue: dynamically-built text lives in the
// arena text pool ([`DocText::Pooled`], `MultilineText`), never in per-node
// owned `String`s, so `DocArena::reset()`'s `clear()` and the arena's drop
// free the node store without walking every node to run destructors on the
// <1% that used to own heap payloads. A new heap-owning variant would silently
// reintroduce that walk on every reset across all surfaces (CLI workers,
// FFI/N-API/WASM thread-local reuse) — this guard makes it a compile error;
// route the payload through the pool instead.
const _: () = assert!(!std::mem::needs_drop::<DocNode>());

// `DocNode` is an AoS node whose size the whole memory strategy is load-bearing on:
// the arena's node store is walked linearly at render, so the AoS layout's cache locality is
// the point (SoA and per-variant boxing both measured worse), and shrinking the node has been
// refuted repeatedly (a smaller node loses on this traversal-bound engine — the bumpalo lesson).
// A variant that bloats it would silently regress that locality with no other signal, so pin the
// size — a change here is a deliberate decision, not an accident. The size is pointer-width
// dependent (the `Align { n: usize }` and `DocText::Static(&str)` fat-pointer payloads), so it is
// pinned per target: 32 B on 64-bit (the native flagship) and 16 B on wasm32 (the shipped WASM
// bundles, where the locality/allocator budget matters most).
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<DocNode>() == 32);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(size_of::<DocNode>() == 16);

/// A command in the printer's command stack.
///
/// Holds a `DocId` index, making it `Copy` with no lifetime parameter.
#[derive(Debug, Clone, Copy)]
pub struct ArenaCommand {
    pub indent: usize,
    pub mode: Mode,
    pub doc: DocId,
}

impl ArenaCommand {
    /// Create a command with the same context but a different doc.
    #[inline]
    pub fn with_doc(&self, doc: DocId) -> Self {
        Self { doc, ..*self }
    }

    /// Create a command with incremented indent.
    #[inline]
    pub fn indented(&self, doc: DocId) -> Self {
        Self {
            indent: self.indent + 1,
            doc,
            ..*self
        }
    }

    /// Create a command with decremented indent.
    #[inline]
    pub fn dedented(&self, doc: DocId) -> Self {
        Self {
            indent: self.indent.saturating_sub(1),
            doc,
            ..*self
        }
    }

    /// Create a command with absolute indent level.
    #[inline]
    pub fn with_indent(&self, indent: usize, doc: DocId) -> Self {
        Self {
            indent,
            doc,
            ..*self
        }
    }

    /// Create a command with a specific mode.
    #[inline]
    pub fn with_mode(&self, mode: Mode, doc: DocId) -> Self {
        Self { mode, doc, ..*self }
    }
}

/// Inline-backed render work-list. The renderers run many times per file (CSS
/// per declaration/value, Svelte per template expression), each spinning up a
/// fresh command stack from empty — so a `SmallVec` keeps the common small
/// sub-render fully on the stack (no heap allocation), mirroring the fits path's
/// `SmallVec<[(DocId, Mode); 16]>`. `N = 8` is the measured knee of the
/// per-call high-water distribution (≈89% of CSS top-level renders and ≈99.7%
/// of the high-frequency single-doc sub-renders stay inline — measured
/// before the tail-continuation rewrite, which only lowered stack transit, so
/// the knee holds a fortiori); its 128-byte inline footprint matches the fits
/// stack and the `DocBuf` convention. Top-level renders additionally borrow
/// the arena-pooled instance (`borrow_render_commands_scratch`) so their spill
/// capacity warms once per arena instead of re-allocating per rendered piece.
pub(super) type CmdStack = SmallVec<[ArenaCommand; 8]>;

/// Inline-backed pending `line_suffix` buffer. Line suffixes are sparse — the
/// measured high-water never exceeds 1 — so `N = 4` is generous headroom at a
/// 64-byte inline footprint, keeping even the rare suffix push off the heap.
pub(super) type LineSuffixBuf = SmallVec<[ArenaCommand; 4]>;

/// Sentinel cache values for the `arena_fits` flat-width fast-path. A cell holds
/// either a real break-free flat width, or one of these two sentinels — the top
/// two `u32` values, mirroring the `u16` text-width sentinels in `doc::types`
/// (`TEXT_WIDTH_HAS_NEWLINE` / `TEXT_WIDTH_NOT_COMPUTED`). Packing the cache as
/// `u32` (vs an 8-byte enum) halves its footprint — one `u32` per doc node, ~4
/// nodes per source byte — which matters most for the memory-constrained WASM
/// target. (A further `u16` narrowing was measured and rejected: instructions
/// +0.26% on the fits-memo path, and WASM steady high-water +2 pages — the
/// halved realloc-size sequence fragments under talc's binning.)
///
/// A real width aliasing a sentinel would need a ~4 GB-wide break-free flat
/// subtree, which is unreachable on real source; and even then it is
/// correctness-safe, not wrong output — a `FLAT_WIDTH_BREAKS` alias just defers
/// that node to the walk (which sums the same width), and a `FLAT_WIDTH_UNKNOWN`
/// alias just recomputes it. So no cap on stored widths is needed.
pub(super) const FLAT_WIDTH_UNKNOWN: u32 = u32::MAX;
pub(super) const FLAT_WIDTH_BREAKS: u32 = u32::MAX - 1;

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

/// The eager width-cache policy for doc text: pool-stored text
/// ([`DocText::Pooled`], `MultilineText` first lines), verbatim source slices
/// ([`DocArena::source_span`]), and `text()` statics (amortized through the
/// arena's static cache — measured once per unique string, not per
/// node) **always** cache a real width or the newline sentinel at build — so
/// every width query (the fits walk, `render_text`'s column advance) answers
/// from the node alone, the fits path never borrows the pool, and render's
/// per-text byte scan is skipped. The one exception is identifier names
/// ([`DocArena::source_span_ident`]) and interner `Symbol`s: high-frequency,
/// newline-free, and rarely fits-measured, so for them a per-node build-time
/// scan costs more than it saves (measured both ways — eager per-ident width
/// was ~+1.1% on identifier-dense corpora; eager everything-else was
/// −0.6..−0.8% on every mixed corpus).
///
/// The measured width is clamped below the sentinels. Unlike the `u32`
/// flat-width cache above (where aliasing needs a ~4 GB subtree and is benign
/// anyway), a `u16` alias is reachable — a single-line non-ASCII text ≥65,534
/// columns — and `as u16` alone would be wrong twice over: 65,535 aliases
/// `TEXT_WIDTH_HAS_NEWLINE` (fits would treat the line as ending inside the
/// text) and ≥65,536 wraps (a huge text cached as narrow → "always fits").
/// Clamping is verdict-preserving: every fits comparison is against a print
/// width orders of magnitude below the clamp, so "65,533" and the true width
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
/// [`visual_width`](crate::printing::visual_width): on an all-ASCII slice the
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
    if s.len() > FUSED_WIDTH_SCAN_MAX {
        return pooled_text_width_scanned(s);
    }
    let mut width = 0usize;
    for &b in s.as_bytes() {
        match b {
            b'\n' => return TEXT_WIDTH_HAS_NEWLINE,
            b'\t' => width += TAB_WIDTH,
            0x00..=0x7f => width += 1,
            _ => return pooled_text_width_scanned(s),
        }
    }
    width.min(TEXT_WIDTH_NOT_COMPUTED as usize - 1) as u16
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
        visual_width(s, TAB_WIDTH).min(TEXT_WIDTH_NOT_COMPUTED as usize - 1) as u16
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
    /// Pooled top-level render work buffers: the render loop's pending-command
    /// stack and its deferred line-suffix buffer, borrowed for the duration of
    /// one top-level render (`render_doc_iterative`) so their spill capacity
    /// warms once per arena instead of re-allocating per rendered piece.
    /// Sub-renders (fill segments, line-suffix flushes) construct their own
    /// inline `SmallVec`s — measured allocation-free — and never borrow these,
    /// so the held `RefMut` is exclusive by construction (a violated nesting
    /// assumption panics loudly rather than corrupting). Cleared at each
    /// borrow; capacity survives `reset()`.
    render_commands_scratch: RefCell<CmdStack>,
    line_suffix_scratch: RefCell<LineSuffixBuf>,
    /// Parked whole-source line-break table backing the per-file
    /// `build_line_breaks` in each `format_in` — taken (moved out), filled,
    /// and parked back cleared with capacity retained, like `render_scratch`.
    line_breaks_scratch: Cell<Vec<u32>>,
    /// Free-list of reusable [`DocBuf`] assembly buffers for the wide-list doc
    /// builders (statement lists, object/array/call-arg lists, member chains).
    /// A builder assembling a variable-length parts list `acquire`s a cleared
    /// buffer (with retained heap capacity from a prior spill) and `release`s it
    /// on scope exit via the [`PooledDocBuf`] guard. A recursion-safe
    /// pop-or-new / clear-and-push-on-release pool self-sizes to the max
    /// concurrent-live buffers (~30 for real code), turning the per-spill
    /// `SmallVec` malloc/free churn into a handful of long-lived reused
    /// allocations. Retained across `reset()` (reused across files), like the
    /// other scratches; only ever affects allocation, never output.
    docbuf_pool: RefCell<Vec<DocBuf>>,
    /// Memoized `will_break(id)` results, indexed by `DocId`. Lazily extended to
    /// match `nodes`; sound because nodes are append-only and the arena is
    /// per-format, so a node's `will_break` value never changes once it exists.
    will_break_cache: RefCell<Vec<Option<bool>>>,
    /// Memoized flat-mode subtree widths for the `arena_fits` fast-path, indexed
    /// by `DocId`. Lazily extended like `will_break_cache`; valid per-format
    /// (depends only on the fixed `TAB_WIDTH` + the interner, both fixed for a
    /// render).
    flat_width_cache: RefCell<Vec<u32>>,
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
    /// The address is a link-time constant, so the slot hash folds per call
    /// site; a collision evict just re-measures and re-allocs (measured rare —
    /// the unique-static population is ~180 on real corpora, ≪ the slot count,
    /// and evicts are ≤0.7% of `text()` calls). `Cell` (no borrow flag) —
    /// probes never alias the `RefCell` stores. Inline by design: the arena
    /// lives on the stack or in a thread-local and is only ever borrowed,
    /// never moved after construction, so the array adds no per-use
    /// indirection.
    static_cache: [Cell<StaticSlot>; STATIC_CACHE_SLOTS],
    /// The current document's format generation, keying the validity of the
    /// interned node halves in `static_cache`, the singleton cells
    /// (`empty_node`, `line_nodes`, `line_suffix_boundary_node`,
    /// `break_parent_node`), and the `symbol_nodes` table. Starts
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
    /// Per-document interned [`DocText::Symbol`] nodes, direct-indexed by the
    /// symbol id — the Symbol analog of the static cache's node half. A Symbol
    /// node is position-free at render (the id fully determines the resolved
    /// text) and carries no per-use state, so every `symbol(id)` within one
    /// document can return one shared node. Ids are small dense integers from
    /// the per-document interner (vocabulary = element/attribute names,
    /// typically tens), so a direct-indexed table needs no hash probe. Each
    /// entry is `(generation, id)`, valid iff the generation matches
    /// [`Self::format_gen`]; lazily grown to the document's max id + 1, with
    /// capacity retained across `reset()` (the gen bump invalidates in O(1)).
    symbol_nodes: RefCell<Vec<(u32, DocId)>>,
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

/// 512 slots (× 24 B on 64-bit = 12 KB inline). Kept in lockstep with the
/// slot-hash shift — see the assert below; comfortably above the unique-static
/// population (measured ~165–190 distinct statics across real corpora), so
/// steady-state collisions are rare (evicts ≤0.7% of `text()` calls; a 1024-slot
/// A/B only halved the colliding-slot count, not worth doubling the array).
const STATIC_CACHE_SLOTS: usize = 512;

// The slot index is the TOP 9 BITS of the 64-bit multiplicative hash
// (`>> 55` in `static_width`), which is provably `< 512` — that both elides
// the array bounds check and hard-couples the shift to the slot count. This
// assert makes changing one without the other a compile error.
const _: () = assert!(STATIC_CACHE_SLOTS == 1 << 9);

impl DocArena {
    /// Create a new empty arena.
    pub fn new() -> Self {
        Self {
            nodes: RefCell::new(Vec::new()),
            children: RefCell::new(Vec::new()),
            text_pool: RefCell::new(String::new()),
            pool_scratch: Cell::new(String::new()),
            render_scratch: Cell::new(String::new()),
            render_commands_scratch: RefCell::new(SmallVec::new()),
            line_suffix_scratch: RefCell::new(SmallVec::new()),
            line_breaks_scratch: Cell::new(Vec::new()),
            docbuf_pool: RefCell::new(Vec::new()),
            will_break_cache: RefCell::new(Vec::new()),
            flat_width_cache: RefCell::new(Vec::new()),
            static_cache: [const { Cell::new(StaticSlot::EMPTY) }; STATIC_CACHE_SLOTS],
            format_gen: Cell::new(1),
            empty_node: Cell::new((0, DocId(0))),
            line_nodes: [const { Cell::new((0, DocId(0))) }; 4],
            line_suffix_boundary_node: Cell::new((0, DocId(0))),
            break_parent_node: Cell::new((0, DocId(0))),
            symbol_nodes: RefCell::new(Vec::new()),
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
            render_commands_scratch: RefCell::new(SmallVec::new()),
            line_suffix_scratch: RefCell::new(SmallVec::new()),
            line_breaks_scratch: Cell::new(Vec::new()),
            docbuf_pool: RefCell::new(Vec::new()),
            // The fitting memos top out at `nodes.len()` (~= `estimated_nodes`),
            // growing from 0 via repeated `resize(nodes.len(), …)`; pre-reserve
            // to absorb those reallocs. Only capacity changes — never values.
            will_break_cache: RefCell::new(Vec::with_capacity(estimated_nodes)),
            flat_width_cache: RefCell::new(Vec::with_capacity(estimated_nodes)),
            static_cache: [const { Cell::new(StaticSlot::EMPTY) }; STATIC_CACHE_SLOTS],
            format_gen: Cell::new(1),
            empty_node: Cell::new((0, DocId(0))),
            line_nodes: [const { Cell::new((0, DocId(0))) }; 4],
            line_suffix_boundary_node: Cell::new((0, DocId(0))),
            break_parent_node: Cell::new((0, DocId(0))),
            symbol_nodes: RefCell::new(Vec::new()),
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
            self.symbol_nodes.get_mut().clear();
            self.format_gen.set(1);
        } else {
            self.format_gen.set(next);
        }
        self.nodes.get_mut().clear();
        self.children.get_mut().clear();
        self.text_pool.get_mut().clear();
        self.will_break_cache.get_mut().clear();
        self.flat_width_cache.get_mut().clear();
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
    /// Hot path (92–95% of calls on real corpora): one slot load + ptr/len/gen
    /// compare (the address is a link-time constant, so the slot hash folds
    /// per call site). The miss path — first use this document, first
    /// sighting ever, or collision evict — allocs and restamps in the cold
    /// helper.
    #[inline]
    pub fn text(&self, s: &'static str) -> DocId {
        let ptr = s.as_ptr() as usize;
        // Hash in u64: usize is 32-bit on wasm32, where the Fibonacci constant
        // and the top-9-bit shift would overflow. The `>> 55` keeps the top 9
        // bits ⇒ index < 512, locked to `STATIC_CACHE_SLOTS` by the assert at
        // its definition (and eliding the bounds check below).
        let slot_i = ((ptr as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 55) as usize;
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
        let first = s.split('\n').next().unwrap_or("");
        let first_width =
            visual_width(first, TAB_WIDTH).min(TEXT_WIDTH_NOT_COMPUTED as usize - 1) as u16;
        let span = self.pool_push(s);
        self.alloc(DocNode::MultilineText { span, first_width })
    }

    /// Create a pooled-text doc (via [`Self::text_pooled`]) for a *line comment*
    /// (`// …` or hashbang) — text whose content runs to end-of-line.
    ///
    /// Identical to [`Self::text_pooled`] for output. Under the `swallow_check`
    /// feature, while the check is enabled ([`super::swallow`]) it additionally
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
    /// at render via a [`super::SourceTextResolver`]. Identifier names use
    /// [`Self::source_span_ident`] instead (deferred width — the opposite
    /// tradeoff).
    #[inline]
    pub fn source_span(&self, span: Span, source: &str) -> DocId {
        let w = pooled_text_width(span.extract(source));
        self.alloc(DocNode::Text(DocText::SourceSpan(span, w)))
    }

    /// [`Self::source_span`] for a slice the caller guarantees is newline-free
    /// (identifier names): skips the width precompute entirely — no source read
    /// at build. Width is measured on demand at the first `fits()` touch
    /// (memoized) — the deferral kept only for identifier names and `Symbol`s
    /// (high-frequency, rarely fits-measured); a non-ASCII name measures the
    /// same value lazily as eagerly, so output is unaffected. Do NOT use for
    /// text that can contain `\n` — the newline sentinel would be missed.
    #[inline]
    pub fn source_span_ident(&self, span: Span) -> DocId {
        self.alloc(DocNode::Text(DocText::SourceSpan(
            span,
            TEXT_WIDTH_NOT_COMPUTED,
        )))
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
    /// prints verbatim (the instrument false-positive [`tag_comment_doc`] can't see, because
    /// nothing walks the transform). This copies the tag across the rebuild.
    ///
    /// Sound for the binary-search invariant: the only nodes ever tagged are comment doc roots
    /// (a `SourceSpan` text — left untouched by the transform, so it never reaches here — a
    /// `MultilineText`, or a multi-child `Concat`), and both re-allocated kinds are replaced by
    /// a **fresh** allocation, never an interned/short-circuited id — so whenever this pushes,
    /// `new` is the highest id so far and `comment_docs` stays sorted ascending (see the field
    /// doc + [`tag_comment_doc`]). Safe against double-counting: the transform returns the new
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
    /// and [`Self::break_parent`]: each is a node with no per-use state, so
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

    /// Create a text doc from a symbol ID (deferred resolution), interned per
    /// document — repeated `symbol(id)` calls within one format return one
    /// shared node instead of allocating per call (Symbol nodes are
    /// position-free at render and carry no per-use state, like `Static` text
    /// and the `Line` singletons; measured ~65–80% of calls dedup on Svelte
    /// corpora — Symbol is Svelte-only in practice).
    #[inline]
    pub fn symbol(&self, id: u32) -> DocId {
        let format_gen = self.format_gen.get();
        {
            let slots = self.symbol_nodes.borrow();
            if let Some(&(node_gen, node_id)) = slots.get(id as usize)
                && node_gen == format_gen
            {
                return node_id;
            }
        }
        self.symbol_miss(id, format_gen)
    }

    /// The cold half of [`Self::symbol`]: alloc this document's node for `id`,
    /// grow the table to cover the id, and stamp the slot (once per distinct
    /// symbol per document).
    #[cold]
    #[inline(never)]
    fn symbol_miss(&self, id: u32, format_gen: u32) -> DocId {
        let node_id = self.alloc(DocNode::Text(DocText::Symbol(id)));
        let mut slots = self.symbol_nodes.borrow_mut();
        let i = id as usize;
        if i >= slots.len() {
            slots.resize(i + 1, (0, DocId(0)));
        }
        slots[i] = (format_gen, node_id);
        node_id
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

    /// Create a conditional group that tries multiple alternative layouts.
    ///
    /// states[0] is tried first (stored as contents), states[1..] stored in expanded_states.
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

    /// Increase indentation for nested doc.
    pub fn indent(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::Indent(doc))
    }

    /// Decrease indentation for doc.
    pub fn dedent(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::Dedent(doc))
    }

    /// Set absolute indentation level for doc.
    pub fn align(&self, n: usize, doc: DocId) -> DocId {
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
    pub fn indent_if_break(&self, doc: DocId, group_id: GroupId, negate: bool) -> DocId {
        self.alloc(DocNode::IndentIfBreak {
            contents: doc,
            group_id,
            negate,
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
    pub fn concat(&self, docs: &[DocId]) -> DocId {
        match docs {
            [] => self.empty(),
            [single] => *single,
            _ => {
                let range = self.alloc_children(docs);
                self.alloc(DocNode::Concat(range))
            }
        }
    }

    /// Create a fill doc for greedy line packing.
    pub fn fill(&self, parts: &[DocId]) -> DocId {
        let range = self.alloc_children(parts);
        self.alloc(DocNode::Fill(range))
    }

    /// Wrap a doc with rendering context.
    pub fn with_context(&self, doc: DocId, context: DocContext) -> DocId {
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
    pub fn will_break(&self, id: DocId) -> bool {
        let nodes = self.nodes.borrow();
        let children = self.children.borrow();
        let mut cache = self.will_break_cache.borrow_mut();
        if cache.len() < nodes.len() {
            cache.resize(nodes.len(), None);
        }
        Self::will_break_memo(id, &nodes, &children, cache.as_mut_slice())
    }

    /// Split into an inline cache probe over an outlined recursive fill: the
    /// same subtree is re-checked far more often than it is first computed, so
    /// the warm path is a load + compare at the call site instead of a full
    /// call.
    #[inline]
    fn will_break_memo(
        id: DocId,
        nodes: &[DocNode],
        children: &[DocId],
        cache: &mut [Option<bool>],
    ) -> bool {
        if let Some(cached) = cache[id.index()] {
            return cached;
        }
        Self::will_break_fill(id, nodes, children, cache)
    }

    /// The cold half of [`Self::will_break_memo`]: compute and cache whether a
    /// subtree forces a break. Runs at most once per node; recursion goes back
    /// through the inline probe so warm children never re-enter here.
    #[cold]
    #[inline(never)]
    fn will_break_fill(
        id: DocId,
        nodes: &[DocNode],
        children: &[DocId],
        cache: &mut [Option<bool>],
    ) -> bool {
        let result = match &nodes[id.index()] {
            DocNode::Text(_) => false,
            // Contains hardlines → always breaks (like the `concat([…, hardline, …])` it replaces).
            DocNode::MultilineText { .. } => true,
            DocNode::Line(kind) => matches!(kind, LineKind::Hard | LineKind::Literal),
            DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                Self::will_break_memo(*inner, nodes, children, cache)
            }
            DocNode::Align { contents, .. } => {
                Self::will_break_memo(*contents, nodes, children, cache)
            }
            DocNode::IndentIfBreak { contents, .. } => {
                Self::will_break_memo(*contents, nodes, children, cache)
            }
            DocNode::Group {
                contents,
                should_break,
                ..
            } => *should_break || Self::will_break_memo(*contents, nodes, children, cache),
            DocNode::IfBreak { .. } => false,
            DocNode::Concat(range) | DocNode::Fill(range) => range
                .resolve(children)
                .iter()
                .any(|&kid| Self::will_break_memo(kid, nodes, children, cache)),
            DocNode::WithContext { doc, .. } => Self::will_break_memo(*doc, nodes, children, cache),
            DocNode::LineSuffix(_) => false,
            DocNode::LineSuffixBoundary => false,
            DocNode::BreakParent => true,
        };
        cache[id.index()] = Some(result);
        result
    }

    /// Check if a doc has forced breaks (hardlines only, no should_break groups).
    pub fn has_forced_break(&self, id: DocId) -> bool {
        let nodes = self.nodes.borrow();
        self.has_forced_break_inner(id, &nodes)
    }

    fn has_forced_break_inner(&self, id: DocId, nodes: &[DocNode]) -> bool {
        match &nodes[id.index()] {
            DocNode::Text(_) => false,
            DocNode::MultilineText { .. } => true,
            DocNode::Line(kind) => matches!(kind, LineKind::Hard | LineKind::Literal),
            DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                self.has_forced_break_inner(*inner, nodes)
            }
            DocNode::Align { contents, .. } => self.has_forced_break_inner(*contents, nodes),
            DocNode::IndentIfBreak { contents, .. } => {
                self.has_forced_break_inner(*contents, nodes)
            }
            DocNode::Group { contents, .. } => self.has_forced_break_inner(*contents, nodes),
            DocNode::IfBreak { .. } => false,
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let children = self.children.borrow();
                let kids = range.resolve(&children);
                kids.iter()
                    .any(|&kid| self.has_forced_break_inner(kid, nodes))
            }
            DocNode::WithContext { doc, .. } => self.has_forced_break_inner(*doc, nodes),
            DocNode::LineSuffix(_) => false,
            DocNode::LineSuffixBoundary => false,
            DocNode::BreakParent => true,
        }
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
        self.can_break_inner(id, &nodes)
    }

    fn can_break_inner(&self, id: DocId, nodes: &[DocNode]) -> bool {
        match &nodes[id.index()] {
            DocNode::Line(_) => true,
            DocNode::Indent(inner) | DocNode::Dedent(inner) => self.can_break_inner(*inner, nodes),
            DocNode::Align { contents, .. } => self.can_break_inner(*contents, nodes),
            DocNode::IndentIfBreak { contents, .. } => self.can_break_inner(*contents, nodes),
            DocNode::Group {
                contents,
                expanded_states,
                ..
            } => {
                if self.can_break_inner(*contents, nodes) {
                    return true;
                }
                if !expanded_states.is_empty() {
                    let children = self.children.borrow();
                    let kids = expanded_states.resolve(&children);
                    if kids.iter().any(|&kid| self.can_break_inner(kid, nodes)) {
                        return true;
                    }
                }
                false
            }
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                ..
            } => self.can_break_inner(*break_doc, nodes) || self.can_break_inner(*flat_doc, nodes),
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let children = self.children.borrow();
                let kids = range.resolve(&children);
                kids.iter().any(|&kid| self.can_break_inner(kid, nodes))
            }
            DocNode::WithContext { doc, .. } => self.can_break_inner(*doc, nodes),
            DocNode::LineSuffix(inner) => self.can_break_inner(*inner, nodes),
            DocNode::MultilineText { .. } => true,
            DocNode::Text(_) | DocNode::LineSuffixBoundary => false,
            DocNode::BreakParent => true,
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
    /// tsv's `contents` *is* state[0], so recursing both is the same thing. Keeping the
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
    /// The two used to be one function that kept prettier's name while quietly doing this,
    /// which is how a multi-line comment in a flattened arrow signature got its newline
    /// deleted. Two questions, two names.
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
            Align(usize, DocId),
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
        }

        let info = {
            let nodes = self.nodes.borrow();
            match &nodes[id.index()] {
                DocNode::Text(_) | DocNode::LineSuffixBoundary => Info::Keep,
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
                DocNode::BreakParent => Info::BreakParent,
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
    #[inline]
    pub fn release_docbuf(&self, mut buf: DocBuf) {
        buf.clear();
        self.docbuf_pool.borrow_mut().push(buf);
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

    /// Borrow the pooled top-level render command stack (cleared here). Held
    /// for the duration of one top-level render; sub-renders use their own
    /// inline locals and never take this borrow.
    #[inline]
    pub(super) fn borrow_render_commands_scratch(&self) -> std::cell::RefMut<'_, CmdStack> {
        let mut scratch = self.render_commands_scratch.borrow_mut();
        scratch.clear();
        scratch
    }

    /// Borrow the pooled top-level line-suffix buffer (cleared here) — the
    /// companion of [`Self::borrow_render_commands_scratch`].
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

    /// Mutably borrow the flat-width cache for the `arena_fits` fast-path.
    #[inline]
    pub(super) fn borrow_flat_width_cache(&self) -> std::cell::RefMut<'_, Vec<u32>> {
        self.flat_width_cache.borrow_mut()
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
/// alloc/dealloc pair per call). Implements [`fmt::Write`] (never errors) so
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
    use super::{TEXT_WIDTH_HAS_NEWLINE, TEXT_WIDTH_NOT_COMPUTED, pooled_text_width};
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
            visual_width(s, TAB_WIDTH).min(TEXT_WIDTH_NOT_COMPUTED as usize - 1) as u16
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
            TEXT_WIDTH_NOT_COMPUTED as usize - 2,
            TEXT_WIDTH_NOT_COMPUTED as usize - 1,
            TEXT_WIDTH_NOT_COMPUTED as usize,
            TEXT_WIDTH_NOT_COMPUTED as usize + 5,
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
