//! Per-node comment injection for the writer's Svelte comment-attach paths.
//!
//! The `Value`-oracle path attaches acorn's leading/trailing comments to nodes
//! by mutating the serialized JSON (`tsv_svelte`'s `comment_attachment`), then
//! splices the whole tree. The writer instead consults a precomputed
//! `WriterComments` map at each node's close and emits the assigned comments
//! *fused* (final char space) — so a comment-bearing `<script>` `Program` or
//! template expression serializes directly, no intermediate `serde_json::Value`.
//!
//! **Why this works regardless of child-visit order.** acorn's attach inserts
//! `leadingComments`/`trailingComments` as *appended* object keys (after a
//! node's own declaration-order fields, via `preserve_order`), so they always
//! serialize last within a node. The DFS's child-visit order (which diverges
//! from field-emission order for a handful of TS node types) only decides
//! *which* node a comment lands on — not where within that node it emits. So a
//! per-node-span map, consulted at each close, reproduces the byte layout
//! exactly. The map is keyed by the node's byte `(start, end)` plus its type,
//! the type disambiguating the one same-span wrapper (`ChainExpression` vs the
//! call/member it wraps — acorn assigns the comments to the wrapper).

use std::cell::Cell;
use std::collections::HashMap;
use tsv_lang::{JsonWriter, LocationMapper};

/// A single comment to emit inline as a `leadingComments`/`trailingComments`
/// element: `{type, value}` for the synthetic preceding-HTML `Line` comment,
/// `{type, value, start, end}` for an ordinary attached comment (byte positions,
/// translated to final char space at emit).
struct WireComment {
    /// `true` → `"Block"`, `false` → `"Line"`.
    is_block: bool,
    /// The comment text (delimiters stripped, block indentation normalized —
    /// `get_comment_value`).
    value: String,
    /// Byte `(start, end)`, or `None` for the preceding-HTML `Line` comment
    /// (which Svelte emits without positions).
    span: Option<(u32, u32)>,
}

/// The comments attached to one node.
struct NodeComments {
    node_type: Box<str>,
    /// Claimed by the first type-matching close, so two distinct nodes sharing a
    /// byte span and type (a shorthand destructuring default's `key` and
    /// `value.left`; a `ChainExpression` wrapper vs its inner call/member) each
    /// take at most one entry — the one the DFS actually attached to, which
    /// always closes first among such siblings.
    consumed: Cell<bool>,
    leading: Vec<WireComment>,
    trailing: Vec<WireComment>,
    /// Emit `trailingComments` before `leadingComments`. Normally leading comes
    /// first (attach inserts it pre-recursion, trailing post), but the `Program`
    /// with a preceding HTML comment gets its `trailingComments` from the DFS
    /// first, then `leadingComments` appended by the HTML-comment prepend — so
    /// the appended key serializes last.
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
    /// Build the map from an attach-mutated skeleton `Value` — the (byte-space)
    /// JSON the writer emits, run through the acorn comment-attach DFS. For every
    /// node carrying `leadingComments`/`trailingComments`, records its byte span +
    /// type. The comment positions stay in byte space (translated to final char
    /// space at emit).
    #[must_use]
    pub fn from_attached_skeleton(value: &serde_json::Value) -> Self {
        let mut wc = Self::default();
        wc.collect(value);
        wc
    }

    fn collect(&mut self, value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if let (Some(node_type), Some(start), Some(end)) = (
                    map.get("type").and_then(serde_json::Value::as_str),
                    map.get("start").and_then(serde_json::Value::as_u64),
                    map.get("end").and_then(serde_json::Value::as_u64),
                ) {
                    let leading = map.get("leadingComments").map(wire_comments_from_value);
                    let trailing = map.get("trailingComments").map(wire_comments_from_value);
                    if leading.is_some() || trailing.is_some() {
                        // Preserve the skeleton's key order (`preserve_order`):
                        // trailing serializes first only when its key precedes
                        // `leadingComments` in the object.
                        let trailing_first = map
                            .keys()
                            .find(|k| *k == "leadingComments" || *k == "trailingComments")
                            .is_some_and(|k| k == "trailingComments");
                        #[allow(clippy::cast_possible_truncation)]
                        self.insert(
                            node_type,
                            start as u32,
                            end as u32,
                            leading.unwrap_or_default(),
                            trailing.unwrap_or_default(),
                            trailing_first,
                        );
                    }
                }
                for (key, child) in map {
                    if key != "leadingComments" && key != "trailingComments" {
                        self.collect(child);
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for child in arr {
                    self.collect(child);
                }
            }
            _ => {}
        }
    }

    /// Record a node's leading/trailing comments.
    fn insert(
        &mut self,
        node_type: &str,
        start: u32,
        end: u32,
        leading: Vec<WireComment>,
        trailing: Vec<WireComment>,
        trailing_first: bool,
    ) {
        if leading.is_empty() && trailing.is_empty() {
            return;
        }
        self.map
            .entry((start, end))
            .or_default()
            .push(NodeComments {
                node_type: node_type.into(),
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
            .find(|e| !e.consumed.get() && &*e.node_type == node_type)
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
}

/// Convert a `leadingComments`/`trailingComments` JSON array to `WireComment`s.
fn wire_comments_from_value(value: &serde_json::Value) -> Vec<WireComment> {
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };
    arr.iter().filter_map(wire_comment_from_value).collect()
}

/// Convert one comment JSON object (`{type, value[, start, end]}`) to a
/// `WireComment`. The preceding-HTML `Line` comment carries no positions.
#[allow(clippy::cast_possible_truncation)]
fn wire_comment_from_value(value: &serde_json::Value) -> Option<WireComment> {
    let obj = value.as_object()?;
    let is_block = obj.get("type").and_then(serde_json::Value::as_str) == Some("Block");
    let value = obj
        .get("value")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let span = match (
        obj.get("start").and_then(serde_json::Value::as_u64),
        obj.get("end").and_then(serde_json::Value::as_u64),
    ) {
        (Some(start), Some(end)) => Some((start as u32, end as u32)),
        _ => None,
    };
    Some(WireComment {
        is_block,
        value,
        span,
    })
}

/// Emit a `[{…},{…}]` array of wire comments.
fn write_wire_comment_array(w: &mut JsonWriter, comments: &[WireComment], loc: LocationMapper<'_>) {
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
fn write_wire_comment(w: &mut JsonWriter, comment: &WireComment, loc: LocationMapper<'_>) {
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
