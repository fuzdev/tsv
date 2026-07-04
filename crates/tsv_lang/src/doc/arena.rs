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

use std::cell::RefCell;

use crate::Span;
use crate::config::TAB_WIDTH;
use crate::printing::visual_width;

use super::DocBuf;
#[cfg(feature = "swallow_check")]
use super::swallow::swallow_check_enabled;
use super::types::{
    DocContext, DocText, GroupId, LineKind, Mode, PoolSpan, TEXT_WIDTH_HAS_NEWLINE,
    TEXT_WIDTH_NOT_COMPUTED,
};

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

    /// A group that prevents hardline propagation to parent groups
    IsolatedGroup { contents: DocId },
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

/// Sentinel cache values for the `arena_fits` flat-width fast-path. A cell holds
/// either a real break-free flat width, or one of these two sentinels — the top
/// two `u32` values, mirroring the `u16` text-width sentinels in `doc::types`
/// (`TEXT_WIDTH_HAS_NEWLINE` / `TEXT_WIDTH_NOT_COMPUTED`). Packing the cache as
/// `u32` (vs an 8-byte enum) halves its footprint — one `u32` per doc node, ~4
/// nodes per source byte — which matters most for the memory-constrained WASM
/// target.
///
/// A real width aliasing a sentinel would need a ~4 GB-wide break-free flat
/// subtree, which is unreachable on real source; and even then it is
/// correctness-safe, not wrong output — a `FLAT_WIDTH_BREAKS` alias just defers
/// that node to the walk (which sums the same width), and a `FLAT_WIDTH_UNKNOWN`
/// alias just recomputes it. So no cap on stored widths is needed.
pub(super) const FLAT_WIDTH_UNKNOWN: u32 = u32::MAX;
pub(super) const FLAT_WIDTH_BREAKS: u32 = u32::MAX - 1;

/// The eager width-cache policy for dynamic doc text: pool-stored text
/// ([`DocText::Pooled`], `MultilineText` first lines) and verbatim source
/// slices ([`DocArena::source_span`]) **always** cache a real width or the
/// newline sentinel at build — so every width query (the fits walk,
/// `render_text`'s column advance) answers from the node alone, the fits path
/// never borrows the pool, and render's per-text byte scan is skipped. The one
/// exception is identifier names ([`DocArena::source_span_ident`], plus
/// `text()` statics and interner `Symbol`s): high-frequency, newline-free, and
/// rarely fits-measured, so for them the build-time scan costs more than it
/// saves (measured both ways — eager per-ident width was ~+1.1% on
/// identifier-dense corpora; eager everything-else was −0.6..−0.8% on every
/// mixed corpus).
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
#[inline]
fn pooled_text_width(s: &str) -> u16 {
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
    /// Memoized `will_break(id)` results, indexed by `DocId`. Lazily extended to
    /// match `nodes`; sound because nodes are append-only and the arena is
    /// per-format, so a node's `will_break` value never changes once it exists.
    will_break_cache: RefCell<Vec<Option<bool>>>,
    /// Memoized flat-mode subtree widths for the `arena_fits` fast-path, indexed
    /// by `DocId`. Lazily extended like `will_break_cache`; valid per-format
    /// (depends only on the fixed `TAB_WIDTH` + the interner, both fixed for a
    /// render).
    flat_width_cache: RefCell<Vec<u32>>,
    /// Diagnostic side-set: indices of text nodes that are line comments,
    /// recorded by `line_comment_text_pooled` only while the swallow check is
    /// enabled (empty and untouched otherwise). Appended in `alloc` order, so
    /// the vec is sorted ascending — the renderer membership-tests via binary
    /// search. See [`super::swallow`]. Compiled in only under the `swallow_check`
    /// feature, so production builds carry no diagnostic state.
    #[cfg(feature = "swallow_check")]
    line_comment_ids: RefCell<Vec<u32>>,
}

impl DocArena {
    /// Create a new empty arena.
    pub fn new() -> Self {
        Self {
            nodes: RefCell::new(Vec::new()),
            children: RefCell::new(Vec::new()),
            text_pool: RefCell::new(String::new()),
            will_break_cache: RefCell::new(Vec::new()),
            flat_width_cache: RefCell::new(Vec::new()),
            #[cfg(feature = "swallow_check")]
            line_comment_ids: RefCell::new(Vec::new()),
        }
    }

    /// Create an arena with pre-allocated capacity based on source size.
    ///
    /// Heuristic: **~2 doc nodes per source byte**. Measured across the
    /// representative corpus (`tsv_debug arena_stats`, 11.3 K files) the real
    /// density is ~0.57 nodes/byte mean with a p99 of ~1.6 and a max of ~3.5; 2/byte
    /// clears p99 with margin, so only the sub-1% densest files pay an (amortized)
    /// realloc, while the typical file no longer over-reserves ~7× (the prior
    /// 4/byte was pinned to the *max* file). `estimated_children = nodes/2` ⇒
    /// ~1/byte, which clears the children p95 (~0.97). The pre-size only ever sets
    /// `Vec` capacity — never output — so lowering it is byte-identical; the win is
    /// the fresh-arena / first-file / WASM reservation, and the multi-file
    /// `reset()` reuse high-water is bounded by actual usage, so it can only drop.
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
            // The fitting memos top out at `nodes.len()` (~= `estimated_nodes`),
            // growing from 0 via repeated `resize(nodes.len(), …)`; pre-reserve
            // to absorb those reallocs. Only capacity changes — never values.
            will_break_cache: RefCell::new(Vec::with_capacity(estimated_nodes)),
            flat_width_cache: RefCell::new(Vec::with_capacity(estimated_nodes)),
            #[cfg(feature = "swallow_check")]
            line_comment_ids: RefCell::new(Vec::new()),
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
    pub fn reset(&mut self) {
        self.nodes.get_mut().clear();
        self.children.get_mut().clear();
        self.text_pool.get_mut().clear();
        self.will_break_cache.get_mut().clear();
        self.flat_width_cache.get_mut().clear();
        #[cfg(feature = "swallow_check")]
        self.line_comment_ids.get_mut().clear();
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

    /// Create a text doc from a static string (zero allocation).
    ///
    /// Never precomputes width. Static strings are short ASCII punctuation,
    /// keywords, and operators — `visual_width()`'s ASCII byte-count fast path
    /// measures them on demand for less than the caching would cost.
    #[inline]
    pub fn text(&self, s: &'static str) -> DocId {
        self.alloc(DocNode::Text(DocText::Static(s, TEXT_WIDTH_NOT_COMPUTED)))
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
    /// (memoized), exactly like [`Self::text`]'s never-precompute policy; a
    /// non-ASCII name measures the same value lazily as eagerly, so output is
    /// unaffected. Do NOT use for text that can contain `\n` — the newline
    /// sentinel would be missed.
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

    /// Create an empty doc that produces no output.
    #[inline]
    pub fn empty(&self) -> DocId {
        self.alloc(DocNode::Text(DocText::Static("", 0)))
    }

    /// Create a text doc from a symbol ID (deferred resolution).
    #[inline]
    pub fn symbol(&self, id: u32) -> DocId {
        self.alloc(DocNode::Text(DocText::Symbol(id)))
    }

    /// Create a normal line break (space if fits, newline if doesn't).
    #[inline]
    pub fn line(&self) -> DocId {
        self.alloc(DocNode::Line(LineKind::Normal))
    }

    /// Create a soft line that disappears in flat mode.
    #[inline]
    pub fn softline(&self) -> DocId {
        self.alloc(DocNode::Line(LineKind::Soft))
    }

    /// Create a hard line break (always breaks).
    #[inline]
    pub fn hardline(&self) -> DocId {
        self.alloc(DocNode::Line(LineKind::Hard))
    }

    /// Create a literal line break (just newline, no indentation).
    #[inline]
    pub fn literalline(&self) -> DocId {
        self.alloc(DocNode::Line(LineKind::Literal))
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

    /// Force pending LineSuffix content to be flushed.
    pub fn line_suffix_boundary(&self) -> DocId {
        self.alloc(DocNode::LineSuffixBoundary)
    }

    /// Force parent group to break.
    pub fn break_parent(&self) -> DocId {
        self.alloc(DocNode::BreakParent)
    }

    /// Create an isolated group that prevents hardline propagation.
    pub fn isolated_group(&self, doc: DocId) -> DocId {
        self.alloc(DocNode::IsolatedGroup { contents: doc })
    }

    //
    // Convenience builders
    //

    /// Build a doc from items with a static string separator between them.
    pub fn join(&self, docs: impl IntoIterator<Item = DocId>, separator: &'static str) -> DocId {
        let iter = docs.into_iter();
        let (lower, _) = iter.size_hint();
        // Shared inline buffer (N=8), matching `join_doc`: the joined parts (2n-1
        // for n items) stay off the heap for the common small list. Call sites join
        // arg/param/specifier/value lists (≥1 item), so this is never the
        // always-empty no-op the SmallVec sweep warns about.
        let mut parts = DocBuf::with_capacity(lower.saturating_mul(2).saturating_sub(1));
        for (i, doc) in iter.enumerate() {
            if i > 0 {
                parts.push(self.text(separator));
            }
            parts.push(doc);
        }
        // `concat` short-circuits empty → `empty()` and single → the element.
        self.concat(&parts)
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
            DocNode::IsolatedGroup { .. } => false,
            DocNode::LineSuffix(_) => false,
            DocNode::LineSuffixBoundary => false,
            DocNode::BreakParent => true,
        };
        cache[id.index()] = Some(result);
        result
    }

    /// Like `will_break`, but also traverses into `IsolatedGroup`.
    ///
    /// Use this for doc analysis (e.g., chain expansion decisions) where
    /// rendering isolation is irrelevant — we need to know if the content
    /// actually contains forced breaks regardless of group isolation.
    pub fn will_break_deep(&self, id: DocId) -> bool {
        let nodes = self.nodes.borrow();
        self.will_break_deep_inner(id, &nodes)
    }

    fn will_break_deep_inner(&self, id: DocId, nodes: &[DocNode]) -> bool {
        match &nodes[id.index()] {
            DocNode::IsolatedGroup { contents, .. } => self.will_break_deep_inner(*contents, nodes),
            DocNode::Text(_) => false,
            DocNode::MultilineText { .. } => true,
            DocNode::Line(kind) => matches!(kind, LineKind::Hard | LineKind::Literal),
            DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                self.will_break_deep_inner(*inner, nodes)
            }
            DocNode::Align { contents, .. } => self.will_break_deep_inner(*contents, nodes),
            DocNode::IndentIfBreak { contents, .. } => self.will_break_deep_inner(*contents, nodes),
            DocNode::Group {
                contents,
                should_break,
                ..
            } => *should_break || self.will_break_deep_inner(*contents, nodes),
            DocNode::IfBreak { .. } => false,
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let children = self.children.borrow();
                let kids = range.resolve(&children);
                kids.iter()
                    .any(|&kid| self.will_break_deep_inner(kid, nodes))
            }
            DocNode::WithContext { doc, .. } => self.will_break_deep_inner(*doc, nodes),
            DocNode::LineSuffix(_) => false,
            DocNode::LineSuffixBoundary => false,
            DocNode::BreakParent => true,
        }
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
            DocNode::IsolatedGroup { .. } => false,
            DocNode::LineSuffix(_) => false,
            DocNode::LineSuffixBoundary => false,
            DocNode::BreakParent => true,
        }
    }

    /// Check if a doc can break (contains any line elements).
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
            DocNode::IsolatedGroup { contents, .. } => self.can_break_inner(*contents, nodes),
            DocNode::LineSuffix(inner) => self.can_break_inner(*inner, nodes),
            DocNode::MultilineText { .. } => true,
            DocNode::Text(_) | DocNode::LineSuffixBoundary => false,
            DocNode::BreakParent => true,
        }
    }

    /// Remove all line breaks from a doc, forcing it to stay on a single line.
    /// Creates new nodes; old nodes remain in the arena (they're just unused).
    pub fn remove_lines(&self, id: DocId) -> DocId {
        // Extract node info while borrowing, then release borrow before allocating.
        // This pattern avoids RefCell conflicts since alloc() needs borrow_mut().
        enum Info {
            Keep, // Return id unchanged
            MultilineText(String),
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
            IsolatedGroup(DocId),
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
                // Flatten: drop the internal hardlines (→ empty) and join the
                // lines with no separator — identical to remove_lines over the
                // per-line `concat([text, hardline, text, …])` this replaces.
                DocNode::MultilineText { span, .. } => {
                    let pool = self.text_pool.borrow();
                    Info::MultilineText(span.slice(&pool).replace('\n', ""))
                }
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
                DocNode::IsolatedGroup { contents } => Info::IsolatedGroup(*contents),
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

        match info {
            Info::Keep => id,
            Info::MultilineText(flat) => self.text_pooled(&flat),
            Info::Line(kind) => match kind {
                LineKind::Normal => self.text(" "),
                LineKind::Soft | LineKind::Hard | LineKind::Literal => self.empty(),
            },
            Info::Indent(inner) => {
                let new_inner = self.remove_lines(inner);
                self.indent(new_inner)
            }
            Info::Dedent(inner) => {
                let new_inner = self.remove_lines(inner);
                self.dedent(new_inner)
            }
            Info::Align(n, contents) => {
                let new_contents = self.remove_lines(contents);
                self.align(n, new_contents)
            }
            Info::Group {
                contents,
                expanded_states,
                id: group_id,
                should_break,
            } => {
                let flat_contents = self.remove_lines(contents);
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
                        let new_kids: DocBuf =
                            kids.into_iter().map(|kid| self.remove_lines(kid)).collect();
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
            Info::IsolatedGroup(contents) => {
                let new_contents = self.remove_lines(contents);
                self.isolated_group(new_contents)
            }
            Info::IfBreakFlat(flat_doc) => self.remove_lines(flat_doc),
            Info::IndentIfBreakContents(contents) => self.remove_lines(contents),
            Info::Concat(kids) => {
                let flattened: DocBuf =
                    kids.into_iter().map(|kid| self.remove_lines(kid)).collect();
                self.concat(&flattened)
            }
            Info::Fill(kids) => {
                // Fill becomes regular concat when flattened
                let flattened: DocBuf =
                    kids.into_iter().map(|kid| self.remove_lines(kid)).collect();
                self.concat(&flattened)
            }
            Info::WithContext(doc, context) => {
                let new_doc = self.remove_lines(doc);
                self.with_context(new_doc, context)
            }
            Info::LineSuffix(inner) => {
                let new_inner = self.remove_lines(inner);
                self.line_suffix(new_inner)
            }
            Info::BreakParent => self.empty(),
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

    /// Mutably borrow the flat-width cache for the `arena_fits` fast-path.
    #[inline]
    pub(super) fn borrow_flat_width_cache(&self) -> std::cell::RefMut<'_, Vec<u32>> {
        self.flat_width_cache.borrow_mut()
    }

    /// Estimate output buffer capacity (bytes) for the rendered string.
    ///
    /// Called on the fully-built arena, so `nodes.len()` is the final node count.
    /// Measured across the representative corpus (`tsv_debug arena_stats`, 11.3 K
    /// files) the rendered output is **~1.9 bytes per doc node** (aggregate
    /// 1.888×nodes = 1.00×source), so `nodes.len() * 2` reserves the output with a
    /// few-percent headroom — big files (which dominate the `realloc` memcpy cost)
    /// carry the aggregate ratio and so fit in one reservation, while only small,
    /// high-ratio files pay an (amortized, cheap) realloc. This pre-sizes the
    /// render `String` and avoids the geometric `realloc`+memcpy chain a small
    /// default capacity pays (~2–3 grows per format); output writes are ~8% of the
    /// format profile, so eliminating those memcpys is a native + WASM wall lever.
    ///
    /// The prior `nodes.len() / 4` was calibrated to the old 4-nodes/byte pre-size
    /// (then `nodes/4 ≈ source ≈ output`) and under-provisioned the real output
    /// ~3.8× → every format reallocated 2–3 times.
    ///
    /// Floor: 256 bytes (tiny inputs). Ceiling: 1 GiB — a pure sanity backstop
    /// that no real format approaches (the estimate tracks the actual node count),
    /// raised from the old 1 MiB which capped any file whose output exceeded 1 MB
    /// and re-introduced reallocs on large files.
    #[inline]
    pub fn estimated_output_capacity(&self) -> usize {
        (self.nodes.borrow().len() * 2).clamp(256, 1 << 30)
    }
}

impl Default for DocArena {
    fn default() -> Self {
        Self::new()
    }
}
