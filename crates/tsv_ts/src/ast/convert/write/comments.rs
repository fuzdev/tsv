//! Per-node comment injection for the writer's Svelte comment-attach paths.
//!
//! `tsv_svelte`'s comment attachment (`comment_attachment`) runs acorn's
//! leading/trailing attach DFS over a byte-space skeleton tree the writer
//! records while emitting (`SkeletonRecorder`). The writer consults the
//! resulting `WriterComments` map at each node's close and emits the assigned
//! comments *fused* (final char space) — so a comment-bearing `<script>`
//! `Program` or template expression serializes directly, with no intermediate
//! `serde_json::Value` anywhere on the path.
//!
//! **Why this works regardless of child-visit order.** acorn's attach inserts
//! `leadingComments`/`trailingComments` as *appended* object keys (after a
//! node's own declaration-order fields), so they always serialize last within
//! a node. The DFS's child-visit order (which diverges from field-emission
//! order for a handful of TS node types) only decides *which* node a comment
//! lands on — not where within that node it emits. So a per-node-span map,
//! consulted at each close, reproduces the byte layout exactly. The map is
//! keyed by the node's byte `(start, end)` plus its type, the type
//! disambiguating the one same-span wrapper (`ChainExpression` vs the
//! call/member it wraps — acorn assigns the comments to the wrapper).

use std::cell::Cell;
use std::collections::HashMap;
use tsv_lang::{JsonWriter, LocationMapper};

/// A single comment to emit inline as a `leadingComments`/`trailingComments`
/// element: `{type, value}` for the synthetic preceding-HTML `Line` comment,
/// `{type, value, start, end}` for an ordinary attached comment (byte positions,
/// translated to final char space at emit). The attach walk in `tsv_svelte`
/// constructs these for `WriterComments::insert_node`.
pub struct AttachedComment {
    /// `true` → `"Block"`, `false` → `"Line"`.
    pub is_block: bool,
    /// The comment text (delimiters stripped, block indentation normalized —
    /// `get_comment_value`).
    pub value: String,
    /// Byte `(start, end)`, or `None` for the preceding-HTML `Line` comment
    /// (which Svelte emits without positions).
    pub span: Option<(u32, u32)>,
}

/// The comments attached to one node.
struct NodeComments {
    node_type: &'static str,
    /// Claimed by the first type-matching close, so two distinct nodes sharing a
    /// byte span and type (a shorthand destructuring default's `key` and
    /// `value.left`; a `ChainExpression` wrapper vs its inner call/member) each
    /// take at most one entry — the one the DFS actually attached to, which
    /// always closes first among such siblings (the attach walk inserts nodes
    /// in visit order, which matches close order for same-span siblings).
    consumed: Cell<bool>,
    leading: Vec<AttachedComment>,
    trailing: Vec<AttachedComment>,
    /// Emit `trailingComments` before `leadingComments`. Normally leading comes
    /// first (attach assigns it pre-recursion, trailing post), but the `Program`
    /// with a preceding HTML comment gets its `trailingComments` from the DFS
    /// first, then `leadingComments` prepended by the HTML-comment step — the
    /// later-touched kind serializes last, matching acorn's appended-key order.
    trailing_first: bool,
}

/// Per-node attached comments consulted by the writer at each node's close.
///
/// Keyed by the node's byte `(start, end)`; the small per-key list is
/// type-discriminated and consume-once (see `NodeComments::consumed`).
#[derive(Default)]
pub struct WriterComments {
    map: HashMap<(u32, u32), Vec<NodeComments>>,
}

impl WriterComments {
    /// Record one node's attached comments (the attach walk's output). Nodes
    /// must be inserted in visit order — for two nodes sharing a span *and*
    /// type, the consume-once lookup at emit relies on insertion order
    /// matching close order. No-op when both lists are empty.
    pub fn insert_node(
        &mut self,
        node_type: &'static str,
        start: u32,
        end: u32,
        leading: Vec<AttachedComment>,
        trailing: Vec<AttachedComment>,
        trailing_first: bool,
    ) {
        if leading.is_empty() && trailing.is_empty() {
            return;
        }
        self.map
            .entry((start, end))
            .or_default()
            .push(NodeComments {
                node_type,
                consumed: Cell::new(false),
                leading,
                trailing,
                trailing_first,
            });
    }

    /// Emit this node's `,"leadingComments":[…]` / `,"trailingComments":[…]`
    /// (fused, final char space) if any are attached. Called at each node close.
    pub(super) fn emit(
        &self,
        w: &mut JsonWriter,
        node_type: &str,
        start: u32,
        end: u32,
        loc: LocationMapper<'_>,
    ) {
        let Some(entries) = self.map.get(&(start, end)) else {
            return;
        };
        let Some(node) = entries
            .iter()
            .find(|e| !e.consumed.get() && e.node_type == node_type)
        else {
            return;
        };
        node.consumed.set(true);
        let emit_leading = |w: &mut JsonWriter| {
            if !node.leading.is_empty() {
                w.raw(",\"leadingComments\":");
                write_wire_comment_array(w, &node.leading, loc);
            }
        };
        let emit_trailing = |w: &mut JsonWriter| {
            if !node.trailing.is_empty() {
                w.raw(",\"trailingComments\":");
                write_wire_comment_array(w, &node.trailing, loc);
            }
        };
        if node.trailing_first {
            emit_trailing(w);
            emit_leading(w);
        } else {
            emit_leading(w);
            emit_trailing(w);
        }
    }

    /// Debug-only guard, called after an island's fused emit completes: every
    /// collected entry must have been consumed by a node close. An unconsumed
    /// entry means the fused emit never closed a node with the skeleton's
    /// `(start, end, type)` key — skeleton/fused drift that would silently drop
    /// the comment from the wire.
    #[inline]
    pub fn debug_assert_consumed(&self) {
        debug_assert!(
            self.map
                .values()
                .flatten()
                .all(|entry| entry.consumed.get()),
            "WriterComments entry not consumed — skeleton/fused span-key drift would drop a comment"
        );
    }
}

/// Emit a `[{…},{…}]` array of wire comments.
fn write_wire_comment_array(
    w: &mut JsonWriter,
    comments: &[AttachedComment],
    loc: LocationMapper<'_>,
) {
    w.raw("[");
    for (i, comment) in comments.iter().enumerate() {
        if i > 0 {
            w.raw(",");
        }
        write_wire_comment(w, comment, loc);
    }
    w.raw("]");
}

/// Emit one wire comment: `{type, value[, start, end]}`.
fn write_wire_comment(w: &mut JsonWriter, comment: &AttachedComment, loc: LocationMapper<'_>) {
    w.raw("{\"type\":\"");
    w.raw(if comment.is_block { "Block" } else { "Line" });
    w.raw("\",\"value\":");
    w.string(&comment.value);
    if let Some((start, end)) = comment.span {
        w.raw(",\"start\":");
        w.u32(loc.pos(start));
        w.raw(",\"end\":");
        w.u32(loc.pos(end));
    }
    w.raw("}");
}
