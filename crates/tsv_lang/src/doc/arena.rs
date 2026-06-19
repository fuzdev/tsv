//! Arena-based document allocation for efficient Doc tree construction and rendering.
//!
//! Instead of heap-allocating each Doc node individually (Box<Doc>, Vec<Doc>),
//! all nodes are stored in a contiguous `Vec<DocNode>` and referenced by `DocId`
//! (a u32 index). Child lists are stored in a separate flat `Vec<DocId>` and
//! referenced by `ChildRange { start, len }`.
//!
//! Benefits:
//! - No recursive drop (dropping the arena = dropping two Vecs)
//! - No deep cloning (DocId is Copy)
//! - Cache-friendly contiguous storage
//! - Bulk deallocation

use std::cell::RefCell;

use crate::config::TAB_WIDTH;
use crate::printing::visual_width;

use super::types::{
    DocContext, DocText, GroupId, LineKind, Mode, TEXT_WIDTH_HAS_NEWLINE, TEXT_WIDTH_NOT_COMPUTED,
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
    /// Text content to output (static, owned, or symbol)
    Text(DocText),

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
    /// recorded by `line_comment_text_owned` only while the swallow check is
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
            will_break_cache: RefCell::new(Vec::new()),
            flat_width_cache: RefCell::new(Vec::new()),
            #[cfg(feature = "swallow_check")]
            line_comment_ids: RefCell::new(Vec::new()),
        }
    }

    /// Create an arena with pre-allocated capacity based on source size.
    ///
    /// Heuristic: ~4 nodes per source byte for typical formatted code.
    pub fn with_source_size_hint(source_len: usize) -> Self {
        let estimated_nodes = source_len * 4;
        let estimated_children = estimated_nodes / 2;
        Self {
            nodes: RefCell::new(Vec::with_capacity(estimated_nodes)),
            children: RefCell::new(Vec::with_capacity(estimated_children)),
            will_break_cache: RefCell::new(Vec::new()),
            flat_width_cache: RefCell::new(Vec::new()),
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
    /// keywords, and operators — `visual_width()`'s ASCII fast path handles
    /// them in ~3ns, so caching saves nothing vs the construction cost.
    #[inline]
    pub fn text(&self, s: &'static str) -> DocId {
        self.alloc(DocNode::Text(DocText::Static(s, TEXT_WIDTH_NOT_COMPUTED)))
    }

    /// Create a text doc from an owned string.
    ///
    /// Only precomputes width for non-ASCII strings (unicode template text,
    /// formatted string literals). These trigger expensive grapheme segmentation
    /// (~100-500ns) and may be measured multiple times in fits.
    ///
    /// ASCII owned strings use `visual_width()`'s fast path (~3-5ns per visit),
    /// making caching unnecessary.
    #[inline]
    pub fn text_owned(&self, s: String) -> DocId {
        let w = if s.is_ascii() {
            TEXT_WIDTH_NOT_COMPUTED // ASCII is cheap, don't bother
        } else if s.contains('\n') {
            TEXT_WIDTH_HAS_NEWLINE
        } else {
            visual_width(&s, TAB_WIDTH) as u16
        };
        self.alloc(DocNode::Text(DocText::Owned(s, w)))
    }

    /// Create an owned-text doc for a *line comment* (`// …` or hashbang) — text
    /// whose content runs to end-of-line.
    ///
    /// Identical to [`Self::text_owned`] for output. Under the `swallow_check`
    /// feature, while the check is enabled ([`super::swallow`]) it additionally
    /// records the node's id so the renderer can flag any content emitted on the
    /// same physical line after it (silent content loss). Without the feature it
    /// is exactly `text_owned` — no recording, no side-set.
    #[inline]
    pub fn line_comment_text_owned(&self, s: String) -> DocId {
        let id = self.text_owned(s);
        #[cfg(feature = "swallow_check")]
        if super::swallow::swallow_check_enabled() {
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
    pub fn concat(&self, docs: &[DocId]) -> DocId {
        let range = self.alloc_children(docs);
        self.alloc(DocNode::Concat(range))
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
        let mut parts = Vec::with_capacity(lower.saturating_mul(2).saturating_sub(1));
        for (i, doc) in iter.enumerate() {
            if i > 0 {
                parts.push(self.text(separator));
            }
            parts.push(doc);
        }
        if parts.is_empty() {
            self.empty()
        } else {
            self.concat(&parts)
        }
    }

    /// Build a doc from items with a Doc separator between them.
    ///
    /// Since DocId is Copy, no cloning needed for the separator.
    pub fn join_doc(&self, docs: impl IntoIterator<Item = DocId>, separator: DocId) -> DocId {
        let iter = docs.into_iter();
        let (lower, _) = iter.size_hint();
        let mut parts = Vec::with_capacity(lower.saturating_mul(2).saturating_sub(1));
        for (i, doc) in iter.enumerate() {
            if i > 0 {
                parts.push(separator); // Copy, no clone needed!
            }
            parts.push(doc);
        }
        if parts.is_empty() {
            self.empty()
        } else {
            self.concat(&parts)
        }
    }

    /// Join docs with separator, adding trailing separator only when breaking.
    pub fn join_trailing(&self, docs: impl IntoIterator<Item = DocId>, separator: DocId) -> DocId {
        let iter = docs.into_iter();
        let (lower, _) = iter.size_hint();
        let mut parts = Vec::with_capacity(lower.saturating_mul(2));
        for (i, doc) in iter.enumerate() {
            if i > 0 {
                parts.push(separator);
            }
            parts.push(doc);
        }
        if parts.is_empty() {
            return self.empty();
        }
        // Add trailing separator only when breaking
        let trailing = self.extract_trailing_punctuation(separator);
        let empty = self.text("");
        parts.push(self.if_break(trailing, empty));
        self.concat(&parts)
    }

    /// Extract the punctuation part from a separator for trailing comma.
    fn extract_trailing_punctuation(&self, separator: DocId) -> DocId {
        // Extract text info while borrowing, then create node after dropping borrows
        enum TextInfo {
            Static(&'static str),
            Owned(String),
            Symbol(u32),
        }
        let text_info = {
            let nodes = self.nodes.borrow();
            match &nodes[separator.index()] {
                DocNode::Concat(range) => {
                    let children = self.children.borrow();
                    let kids = range.resolve(&children);
                    let mut found = None;
                    for &kid_id in kids {
                        if let DocNode::Text(doc_text) = &nodes[kid_id.index()] {
                            found = Some(match doc_text {
                                DocText::Static(s, _) => TextInfo::Static(s),
                                DocText::Owned(s, _) => TextInfo::Owned(s.clone()),
                                DocText::Symbol(id) => TextInfo::Symbol(*id),
                            });
                            break;
                        }
                    }
                    found
                }
                DocNode::Text(_) => None, // separator itself is fine
                _ => None,
            }
        };
        match text_info {
            Some(TextInfo::Static(s)) => self.text(s),
            Some(TextInfo::Owned(s)) => self.text_owned(s),
            Some(TextInfo::Symbol(id)) => self.symbol(id),
            None => separator,
        }
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

    /// Trailing comma (only appears in break mode).
    #[inline]
    pub fn trailing_comma(&self) -> DocId {
        self.if_break(self.text(","), self.text(""))
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

    fn will_break_memo(
        id: DocId,
        nodes: &[DocNode],
        children: &[DocId],
        cache: &mut [Option<bool>],
    ) -> bool {
        if let Some(cached) = cache[id.index()] {
            return cached;
        }
        let result = match &nodes[id.index()] {
            DocNode::Text(_) => false,
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
            Concat(Vec<DocId>),
            Fill(Vec<DocId>),
            WithContext(DocId, DocContext),
            LineSuffix(DocId),
            BreakParent,
        }

        let info = {
            let nodes = self.nodes.borrow();
            match &nodes[id.index()] {
                DocNode::Text(_) | DocNode::LineSuffixBoundary => Info::Keep,
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
                    Info::Concat(range.resolve(&children).to_vec())
                }
                DocNode::Fill(range) => {
                    let children = self.children.borrow();
                    Info::Fill(range.resolve(&children).to_vec())
                }
                DocNode::WithContext { doc, context } => Info::WithContext(*doc, context.clone()),
                DocNode::LineSuffix(inner) => Info::LineSuffix(*inner),
                DocNode::BreakParent => Info::BreakParent,
            }
        }; // nodes borrow dropped here

        match info {
            Info::Keep => id,
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
                            expanded_states.resolve(&children).to_vec()
                        };
                        let new_kids: Vec<DocId> =
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
                let flattened: Vec<DocId> =
                    kids.into_iter().map(|kid| self.remove_lines(kid)).collect();
                self.concat(&flattened)
            }
            Info::Fill(kids) => {
                // Fill becomes regular concat when flattened
                let flattened: Vec<DocId> =
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

    /// Mutably borrow the flat-width cache for the `arena_fits` fast-path.
    #[inline]
    pub(super) fn borrow_flat_width_cache(&self) -> std::cell::RefMut<'_, Vec<u32>> {
        self.flat_width_cache.borrow_mut()
    }

    /// Estimate output buffer capacity (bytes) for the rendered string.
    ///
    /// Doc trees average ~4 nodes per source byte (see [`with_source_size_hint`]),
    /// and Prettier-conforming output is within ±10% of source. So output bytes
    /// ≈ `nodes.len() / 4`. Used to pre-size the render `String` buffer and avoid
    /// the geometric `realloc` chain that starts from a small default capacity.
    ///
    /// Floor: 256 bytes (matches the old hardcoded default for tiny inputs).
    /// Ceiling: 1 MB (guards against accidental huge initial allocations).
    ///
    /// [`with_source_size_hint`]: Self::with_source_size_hint
    #[inline]
    pub fn estimated_output_capacity(&self) -> usize {
        (self.nodes.borrow().len() / 4).clamp(256, 1 << 20)
    }
}

impl Default for DocArena {
    fn default() -> Self {
        Self::new()
    }
}
