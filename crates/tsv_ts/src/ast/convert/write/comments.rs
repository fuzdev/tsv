//! The **online** comment attach: acorn's leading/trailing assignment, run off
//! the writer's own node open/close events.
//!
//! Svelte attaches comments to an acorn tree with a DFS (`add_comments` in
//! `svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js`, walked by
//! zimmerframe's `for key in node`): a node's **leading** comments are shifted off the
//! queue at node *entry*, its **trailing** ones decided after its children have been
//! walked, and both serialize as *appended* object keys — i.e. last within the node.
//!
//! Those two moments are exactly the writer's `node_header` (`attach_open`) and
//! `close_node`, and the wire emits both lists at the close. So the attach needs no
//! tree and no map: it runs as the emit runs, off one comment queue and the emit's own
//! open stack. What the walk reads about a node's surroundings comes from that stack —
//! `parent.end` is the frame below — and acorn's `is_last_in_body` from the emit itself,
//! marked by the four container writers as they emit that element
//! ([`CommentAttach::mark_last_body_element`], which says why it is a MARK rather than a
//! span read).
//!
//! ⭐ **Child visit order is acorn's by construction.** zimmerframe visits a node's
//! object-valued properties in property-insertion order, and the writer's field order
//! already *is* that order (`write_arrow_function_expression`,
//! `write_function_expression`, `write_new_expression`, `write_call_expression` and the
//! `SwitchCase` / `LabeledStatement` / `MethodDefinition` writers each carry the note).
//! An emit-driven walk cannot disagree with the field order it is itself emitting, so
//! the whole class of "the walk and the wire order a node's children differently" bugs
//! is unrepresentable. The class is real, not hypothetical: a walk that reproduces that
//! order by hand has to get the generic-async arrow right, whose wire order is
//! `[typeParameters, params…, returnType, body]` —
//! `tests/fixtures/typescript/expressions/arrow/async_generic/params_comment_return_type`
//! pins where each of its gaps attaches.
//!
//! Two further hazards are unrepresentable for the same reason. A per-node map keyed by
//! `(start, end, type)` can hold an entry no node ever closes with (a silently dropped
//! comment) and needs a consume-once rule to tell two same-span nodes apart; and a
//! separate pass that builds such a map must be configured *identically* to the emit
//! that reads it (parser variant, schema). Here the node that closes first **is** the
//! node that attached first, and one emission carries the configuration.

use std::cell::RefCell;

use super::CommentMode;
use tsv_lang::{AcornPrefix, Comment, JsonWriter, LocationMapper, Span};

/// What one island declares up front: the comments its canonical parse would have
/// collected, plus the two facts acorn's walk reads about a **root**'s surroundings.
///
/// Everything else the walk needs — child visit order, a non-root's `parent.end`, and
/// which element is last in a container's body — the emit itself supplies, so this is
/// the whole of an island's configuration. Named fields rather than positional
/// arguments because three of the four are an `Option`/`bool` whose meaning is not
/// recoverable from the value: two call sites would otherwise read `None, None, true,
/// None`.
pub struct IslandComments<'a> {
    /// The island's candidate comments, in position order, already filtered to its
    /// window — the set acorn's `onComment` would have pushed during that parse.
    ///
    /// ⚠️ The window's END is the end of the **parsed region**, which is not the
    /// emitted root node's end wherever tsv discards a wrapper the parse had: a JSDoc
    /// cast's `(`…`)` is part of what acorn parsed (its root is a
    /// `ParenthesizedExpression`), and its `)` is exactly what acorn's post-expression
    /// token scan starts past. Deriving the end from the emitted root instead stops the
    /// scan dead on that `)`, and every comment after it is filtered out before the
    /// walk — a `trailingComments` entry canonical emits and tsv then has nowhere to
    /// put. `tsv_svelte`'s `attach_expression` states the rule for the family.
    ///
    /// Each is paired with the source Svelte handed acorn for it — what its wire `value`
    /// is dedented by. Paired rather than carried in a second list, because the pairing is
    /// the only thing that keeps them aligned: read positionally out of two `Vec`s, a
    /// misalignment dedents a comment against a source it was never in and still emits a
    /// well-formed wire.
    ///
    /// ⚠️ The prefix is per COMMENT, not per island, and the two are genuinely different:
    /// a block binding's island is up to **two** parses (the pattern and its `: T`), each
    /// blanking a different span, so one answer for the whole island is wrong for whichever
    /// half it did not come from. [`AcornPrefix::DOCUMENT`] throughout for a standalone
    /// (non-Svelte) parse.
    pub queue: Vec<(&'a Comment, AcornPrefix)>,
    /// The `end` a **root** node sees for its parent — the one thing a closing node asks
    /// about its parent, driving acorn's `node.end !== parent.end` trailing suppression.
    /// (The other thing it asks, acorn's `is_last_in_body`, is a fact about the CHILD and
    /// rides on the child's own frame — see [`CommentAttach::mark_last_body_element`].)
    ///
    /// `None` for an island whose parse root is the node itself; `Some` for a list island
    /// whose canonical parse had a wrapper node that the wire discards (`{@debug a, b}`'s
    /// `SequenceExpression`) but whose `end` still suppresses the last item's trailing
    /// claim.
    pub root_parent_end: Option<u32>,
    /// Run acorn's post-walk root fallback (leftover comments trail the root). Off for a
    /// multi-root list island, whose leftovers belong to the discarded wrapper and die
    /// with it.
    pub root_fallback: bool,
    /// The preceding-HTML comment Svelte reports as the `<script>` `Program`'s first
    /// `leadingComments` entry — a positionless `{type: "Line", value}` it builds itself,
    /// prepended after the fact so a `Program` whose first attach touch was *trailing*
    /// still serializes `trailingComments` first.
    pub html_leading: Option<&'a str>,
}

/// One open node on the emit stack.
struct Frame {
    /// Debug-only: paired against the closing node's type to catch a writer that
    /// bypasses one of the two hooks. Nothing in release reads it, so it is not
    /// carried there — the `#[cfg]` is what makes "this exists for the assert"
    /// structural rather than a comment someone has to believe.
    #[cfg(debug_assertions)]
    node_type: &'static str,
    span: Span,
    /// This node's leading run: `[lead_start, lead_end)` in `attached`.
    lead_start: u32,
    lead_end: u32,
    /// This node is its parent's last `body` / `elements` / `properties` entry —
    /// acorn's `is_last_in_body`, which widens its trailing window.
    is_last_in_body: bool,
}

/// The mutable half. It holds **indices** into the island's comment slice rather than
/// borrows of it, so the borrow stays outside the `RefCell` — a `RefCell<State<'a>>`
/// would make `CommentAttach<'a>` invariant, and with it every `Ctx` and `EmbedWriter`
/// that carries one.
#[derive(Default)]
struct State {
    /// Queue front: the next unassigned comment's index. The queue's end is the
    /// island's own `queue.len()` — the window is settled before the attach is built,
    /// so nothing here resizes it.
    head: usize,
    /// The next node to open is its parent's last body entry (see
    /// [`CommentAttach::mark_last_body_element`]). Consumed by that open.
    next_is_last_body: bool,
    frames: Vec<Frame>,
    /// Every assigned comment index, in assignment order. At a frame's close its leading
    /// run is `attached[lead_start..lead_end]` and its trailing run `attached[lead_end..]`
    /// — each child truncates back to its own `lead_start`, so by the time a node closes
    /// the buffer has shrunk back to exactly that node's own leading run.
    attached: Vec<u32>,
}

/// The online comment attach for one island (one canonical acorn parse).
///
/// Built by `tsv_svelte` per comment-bearing island and handed to the embedded writer
/// as `CommentMode::Attach`; the writer drives it from `attach_open` / `close_node` and
/// it emits each node's `leadingComments` / `trailingComments` in place.
pub struct CommentAttach<'a> {
    source: &'a str,
    /// What the island declared — its comment window and the root's surroundings.
    island: IslandComments<'a>,
    state: RefCell<State>,
}

impl<'a> CommentAttach<'a> {
    /// One island's attach, over the window its caller already settled.
    #[must_use]
    pub fn new(source: &'a str, island: IslandComments<'a>) -> Self {
        Self {
            source,
            island,
            state: RefCell::new(State::default()),
        }
    }

    /// The mode an emission of this island should run under: `Off` where the window
    /// turned out to hold nothing, which is the same wire without the per-node
    /// bookkeeping.
    ///
    /// An island can be built and still be inert, because a writer's cheap "is there a
    /// comment anywhere near here" pre-check is a superset of the window this then
    /// filtered to.
    #[must_use]
    pub fn mode(&self) -> CommentMode<'_> {
        if self.island.queue.is_empty() && self.island.html_leading.is_none() {
            CommentMode::Off
        } else {
            CommentMode::Attach(self)
        }
    }

    /// A node opens: shift every comment before the node's start onto it as leading.
    ///
    /// `#[cold]` because an ordinary (comment-free) emission never reaches this body:
    /// keeping its register pressure out of the inlined node emitters is what makes
    /// `CommentMode` cost one never-taken compare per node.
    // `node_type` feeds only the open/close pairing assert, which is debug-only.
    #[cold]
    #[cfg_attr(not(debug_assertions), allow(unused_variables))]
    pub(super) fn open(&self, node_type: &'static str, span: Span) {
        let st = &mut *self.state.borrow_mut();
        let lead_start = st.attached.len() as u32;
        while self.front(st).is_some_and(|c| c.span.start < span.start) {
            st.attached.push(st.head as u32);
            st.head += 1;
        }
        let lead_end = st.attached.len() as u32;
        st.frames.push(Frame {
            #[cfg(debug_assertions)]
            node_type,
            span,
            lead_start,
            lead_end,
            is_last_in_body: std::mem::take(&mut st.next_is_last_body),
        });
    }

    /// The **next** node to open is its parent's last `body` / `elements` / `properties`
    /// entry — acorn's `is_last_in_body`, which widens that child's trailing window to
    /// several comments across newlines. Called by the four container writers
    /// (`Program`, `BlockStatement`, `ObjectExpression`, `ArrayExpression`) immediately
    /// before they emit that element.
    ///
    /// ⚠️ **It marks the element as it is EMITTED, and must not be re-derived from the
    /// internal AST at the container's open.** A statement's internal span is not always
    /// its wire node's: a decorated `export` (`@dec⏎export class D {}`) has an internal
    /// span starting at the `@` and emits an `ExportNamedDeclaration` starting at
    /// `export`, so a `body.last().span().start` reading names a position no node ever
    /// opens at and the last statement silently stops being last-in-body.
    #[cold]
    pub(super) fn mark_last_body_element(&self) {
        self.state.borrow_mut().next_is_last_body = true;
    }

    /// A node closes: decide its trailing comments, then emit both lists.
    ///
    /// `#[cold]`, as [`open`](Self::open).
    #[cold]
    pub(super) fn close_and_emit(
        &self,
        w: &mut JsonWriter,
        node_type: &'static str,
        span: Span,
        loc: LocationMapper<'_>,
    ) {
        let st = &mut *self.state.borrow_mut();
        let Some(frame) = st.frames.pop() else {
            debug_assert!(false, "attach close without open: {node_type}");
            return;
        };
        #[cfg(debug_assertions)]
        assert!(
            frame.node_type == node_type && frame.span == span,
            "attach open/close mismatch: opened {} ({},{}), closed {} ({},{})",
            frame.node_type,
            frame.span.start,
            frame.span.end,
            node_type,
            span.start,
            span.end,
        );
        debug_assert_eq!(
            frame.lead_end as usize,
            st.attached.len(),
            "a closing node's descendants left entries behind — its trailing run must \
             start exactly at the end of its own leading run"
        );
        debug_assert!(
            !st.next_is_last_body,
            "{node_type} closed with an unconsumed last-body mark — the marked element \
             emitted no node, so the mark would land on an unrelated one"
        );
        let is_root = st.frames.is_empty();
        let parent_end = if is_root {
            self.island.root_parent_end
        } else {
            st.frames.last().map(|f| f.span.end)
        };
        self.attach_trailing(st, span, parent_end, frame.is_last_in_body);
        if is_root && self.island.root_fallback {
            self.attach_root_fallback(st, node_type, span);
        }
        let leading = &st.attached[frame.lead_start as usize..frame.lead_end as usize];
        let trailing = &st.attached[frame.lead_end as usize..];
        let html = if is_root {
            self.island.html_leading
        } else {
            None
        };
        self.emit(w, leading, trailing, html, loc);
        st.attached.truncate(frame.lead_start as usize);
    }

    /// The queue front, or `None` when the island is drained.
    #[inline]
    fn front(&self, st: &State) -> Option<&'a Comment> {
        (st.head < self.island.queue.len()).then(|| self.island.queue[st.head].0)
    }

    /// acorn's post-recursion trailing rule for one node.
    fn attach_trailing(
        &self,
        st: &mut State,
        span: Span,
        parent_end: Option<u32>,
        is_last_in_body: bool,
    ) {
        let Some(first) = self.front(st) else {
            return;
        };
        // `if (parent === undefined || node.end !== parent.end)` — a node ending where
        // its parent ends leaves the claim to the parent.
        if parent_end == Some(span.end) {
            return;
        }
        if is_last_in_body {
            // Last in a body: several trailing comments, newlines allowed between them,
            // stopping at the parent's own end.
            while let Some(c) = self.front(st) {
                if parent_end.is_some_and(|pe| c.span.start >= pe) {
                    break;
                }
                st.attached.push(st.head as u32);
                st.head += 1;
            }
        } else if span.end <= first.span.start {
            // Otherwise at most ONE, and only across a `/^[,) \t]*$/` gap.
            let gap = &self.source[span.end as usize..first.span.start as usize];
            if gap.bytes().all(|b| matches!(b, b',' | b')' | b' ' | b'\t')) {
                st.attached.push(st.head as u32);
                st.head += 1;
            }
        }
    }

    /// acorn's "trailing comments after the root node" special case: whatever is left
    /// trails the root, provided it starts past the root or the root is a `Program`.
    fn attach_root_fallback(&self, st: &mut State, node_type: &'static str, span: Span) {
        let Some(first) = self.front(st) else {
            return;
        };
        if first.span.start < span.end && node_type != "Program" {
            return;
        }
        while st.head < self.island.queue.len() {
            st.attached.push(st.head as u32);
            st.head += 1;
        }
    }

    /// Emit `,"leadingComments":[…]` / `,"trailingComments":[…]` for the node that just
    /// closed (fused, final char space).
    ///
    /// Order mirrors acorn's appended object keys: whichever list the attach touched
    /// **first** serializes first. Leading is assigned at node entry and trailing after
    /// the children, so leading normally wins — except where a node's only attach-time
    /// touch was trailing and its leading run arrives afterwards, which is exactly the
    /// `<script>` `Program` with a preceding HTML comment.
    fn emit(
        &self,
        w: &mut JsonWriter,
        leading: &[u32],
        trailing: &[u32],
        html: Option<&str>,
        loc: LocationMapper<'_>,
    ) {
        if leading.is_empty() && trailing.is_empty() && html.is_none() {
            return;
        }
        if leading.is_empty() && !trailing.is_empty() {
            self.emit_trailing(w, trailing, loc);
            self.emit_leading(w, leading, html, loc);
        } else {
            self.emit_leading(w, leading, html, loc);
            self.emit_trailing(w, trailing, loc);
        }
    }

    fn emit_leading(
        &self,
        w: &mut JsonWriter,
        leading: &[u32],
        html: Option<&str>,
        loc: LocationMapper<'_>,
    ) {
        if leading.is_empty() && html.is_none() {
            return;
        }
        w.raw(",\"leadingComments\":[");
        if let Some(value) = html {
            // Svelte's own `{type: "Line", value}` — no positions.
            w.raw("{\"type\":\"Line\",\"value\":");
            w.string(value);
            w.raw("}");
            if !leading.is_empty() {
                w.raw(",");
            }
        }
        self.write_run(w, leading, loc);
        w.raw("]");
    }

    fn emit_trailing(&self, w: &mut JsonWriter, trailing: &[u32], loc: LocationMapper<'_>) {
        if trailing.is_empty() {
            return;
        }
        w.raw(",\"trailingComments\":[");
        self.write_run(w, trailing, loc);
        w.raw("]");
    }

    /// The comma-separated `{type, value, start, end}` objects of one run.
    fn write_run(&self, w: &mut JsonWriter, run: &[u32], loc: LocationMapper<'_>) {
        for (i, &idx) in run.iter().enumerate() {
            if i > 0 {
                w.raw(",");
            }
            let (comment, prefix) = self.island.queue[idx as usize];
            w.raw("{\"type\":\"");
            w.raw(if comment.is_block { "Block" } else { "Line" });
            w.raw("\",\"value\":");
            w.string(&comment.wire_value(self.source, prefix));
            w.raw(",\"start\":");
            w.u32(loc.pos(comment.span.start));
            w.raw(",\"end\":");
            w.u32(loc.pos(comment.span.end));
            w.raw("}");
        }
    }
}

/// The island's close-out: every `attach_open` owes a `close_node`, and a writer that
/// returns without one leaves a node's comments unemitted.
///
/// Skipped while unwinding — the corpus tools format under `catch_unwind` and report a
/// panic rather than aborting, which a panicking `Drop` would take away from them.
///
/// The whole impl is `#[cfg(debug_assertions)]`, not just the assert inside it: a
/// release build then has no drop glue for `CommentAttach` at all, rather than a
/// per-island TLS read and `RefCell` borrow whose only result is discarded. Same
/// reason [`Frame::node_type`] is `#[cfg]`-gated — "this exists for the assert" is
/// structural where it can be, not a comment someone has to believe.
#[cfg(debug_assertions)]
impl Drop for CommentAttach<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        let st = self.state.borrow();
        assert!(
            st.frames.is_empty() && st.attached.is_empty(),
            "an island finished with {} unclosed node(s) and {} unemitted comment(s)",
            st.frames.len(),
            st.attached.len(),
        );
    }
}
