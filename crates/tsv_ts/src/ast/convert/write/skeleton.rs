//! Skeleton recording: the wire tree's structural index, captured during a
//! byte-space skeleton emit.
//!
//! `tsv_svelte`'s comment-attach paths need the exact node tree the writer
//! emits — every node's type, byte span, and child structure, synthetic
//! wrappers (`ChainExpression`) included — to run acorn's comment-attach DFS.
//! Instead of re-parsing the emitted bytes into a `serde_json::Value` (the
//! retired round-trip: `from_slice` + two `Value`-tree walks dominated a
//! comment-bearing `<script>`'s write cost), the writer records the tree as it
//! emits: `node_header` reports each node open and `close_node` each close
//! (`CommentMode::Record`), and the recorder reconstructs the nesting from
//! that event stream.
//!
//! The product is a flat pre-order `Vec<SkelNode>` where each node stores the
//! index one past its last descendant (`subtree_end`), so a node's direct
//! children are recovered by hopping subtrees — no per-node child vector, no
//! per-node allocation at all.
//!
//! Reconstruction is **structural** (open/close pairing), never span-based:
//! wire spans are not properly nested in general — e.g. a shorthand
//! destructuring default `{a = 1}` gives the `Property`'s `key` `Identifier`
//! a span contained in its *sibling* `value` `AssignmentPattern`'s span, so
//! span containment would misparent it.

use std::cell::{Cell, RefCell};
use tsv_lang::Span;

/// One recorded wire node.
struct SkelNode {
    /// The wire `type` string (the same literal `node_header` emits).
    node_type: &'static str,
    /// Byte-space `start`/`end` (the skeleton emit uses the identity mapper,
    /// so these equal the internal AST spans).
    start: u32,
    end: u32,
    /// One past the index of this node's last descendant (pre-order), set at
    /// close. Direct children of node `i` are found by hopping: `j = i + 1;
    /// while j < subtree_end(i) { child j; j = subtree_end(j); }`.
    subtree_end: u32,
    /// `ArrayExpression` only: the last `elements` entry is a hole (`null`),
    /// so acorn's last-in-body trailing window never fires for its elements.
    last_elem_hole: bool,
}

/// Records the wire node tree during a skeleton emit (`CommentMode::Record`).
///
/// Interior-mutable so the shared `Ctx` can hold `&SkeletonRecorder` while the
/// writer drives it from `node_header`/`close_node`. `finish()` yields the
/// immutable `SkeletonTree` the attach walk reads.
#[derive(Default)]
pub struct SkeletonRecorder {
    nodes: RefCell<Vec<SkelNode>>,
    /// Indices of currently-open nodes (the emit stack).
    open: RefCell<Vec<u32>>,
    /// Indices of completed top-level nodes — one per emitted island item
    /// (a `Program` or expression skeleton has exactly one; an expression
    /// list records one per item).
    roots: RefCell<Vec<u32>>,
    /// Set by the `ArrayExpression` writer just before its close when the
    /// last element is a hole; consumed by the next `close()`.
    pending_hole: Cell<bool>,
}

impl SkeletonRecorder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a node open (every wire node header). `#[cold]` keeps the
    /// recorder machinery (RefCell borrows, Vec pushes) out of the inlined
    /// hot-path node emitters — an ordinary emission pays only the
    /// `CommentMode::Record` discriminant compare, never this body's register
    /// pressure.
    #[cold]
    pub(super) fn open(&self, node_type: &'static str, span: Span) {
        let mut nodes = self.nodes.borrow_mut();
        let idx = nodes.len() as u32;
        nodes.push(SkelNode {
            node_type,
            start: span.start,
            end: span.end,
            subtree_end: 0,
            last_elem_hole: false,
        });
        self.open.borrow_mut().push(idx);
    }

    /// Record a node close. The type and span must mirror the node's open —
    /// the pairing is the structural invariant the tree rests on, so a writer
    /// that bypasses either hook (or mismatches them) trips the debug assert
    /// under the fixture suite. `#[cold]` as on `open`.
    #[cold]
    pub(super) fn close(&self, node_type: &'static str, span: Span) {
        let Some(idx) = self.open.borrow_mut().pop() else {
            debug_assert!(false, "skeleton close without open: {node_type}");
            return;
        };
        let mut nodes = self.nodes.borrow_mut();
        let end = nodes.len() as u32;
        let node = &mut nodes[idx as usize];
        debug_assert!(
            node.node_type == node_type && node.start == span.start && node.end == span.end,
            "skeleton open/close mismatch: opened {} ({},{}), closed {} ({},{})",
            node.node_type,
            node.start,
            node.end,
            node_type,
            span.start,
            span.end,
        );
        node.subtree_end = end;
        node.last_elem_hole = self.pending_hole.take();
        drop(nodes);
        if self.open.borrow().is_empty() {
            self.roots.borrow_mut().push(idx);
        }
    }

    /// Flag the currently-closing `ArrayExpression`'s trailing hole (called by
    /// the `ArrayExpression` writer immediately before its `close_node`).
    pub(super) fn flag_last_elem_hole(&self) {
        self.pending_hole.set(true);
    }

    /// Consume the recorder into the finished tree.
    #[must_use]
    pub fn finish(self) -> SkeletonTree {
        debug_assert!(
            self.open.borrow().is_empty(),
            "skeleton finished with unclosed nodes"
        );
        SkeletonTree {
            nodes: self.nodes.into_inner(),
            roots: self.roots.into_inner(),
        }
    }
}

/// The recorded wire tree: flat pre-order nodes + the top-level root indices.
/// Read-only view for the comment-attach walk.
pub struct SkeletonTree {
    nodes: Vec<SkelNode>,
    roots: Vec<u32>,
}

impl SkeletonTree {
    /// Top-level node indices, in emit order (one per island item).
    #[must_use]
    pub fn roots(&self) -> &[u32] {
        &self.roots
    }

    /// The wire `type` of node `idx`.
    #[must_use]
    pub fn node_type(&self, idx: u32) -> &'static str {
        self.nodes[idx as usize].node_type
    }

    /// Byte-space `start` of node `idx`.
    #[must_use]
    pub fn start(&self, idx: u32) -> u32 {
        self.nodes[idx as usize].start
    }

    /// Byte-space `end` of node `idx`.
    #[must_use]
    pub fn end(&self, idx: u32) -> u32 {
        self.nodes[idx as usize].end
    }

    /// Whether `idx` is an `ArrayExpression` whose last element is a hole.
    #[must_use]
    pub fn last_elem_hole(&self, idx: u32) -> bool {
        self.nodes[idx as usize].last_elem_hole
    }

    /// Direct children of node `idx`, in emit order.
    pub fn children(&self, idx: u32) -> impl Iterator<Item = u32> + '_ {
        let end = self.nodes[idx as usize].subtree_end;
        ChildIter {
            nodes: &self.nodes,
            next: idx + 1,
            end,
        }
    }

    /// The start position of node `idx`'s last direct child, if any.
    #[must_use]
    pub fn last_child_start(&self, idx: u32) -> Option<u32> {
        self.children(idx).last().map(|c| self.start(c))
    }
}

struct ChildIter<'a> {
    nodes: &'a [SkelNode],
    next: u32,
    end: u32,
}

impl Iterator for ChildIter<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.next >= self.end {
            return None;
        }
        let idx = self.next;
        self.next = self.nodes[idx as usize].subtree_end;
        Some(idx)
    }
}
