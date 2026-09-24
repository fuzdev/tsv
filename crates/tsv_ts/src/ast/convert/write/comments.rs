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
//!
//! **A subtree that provably takes no comment is skipped.** Most of a comment-bearing
//! island's nodes sit nowhere near a comment, so at every non-root open the attach asks
//! whether anything in the subtree about to open could take the queue's front comment `c`,
//! and when nothing can it skips the whole subtree — no frame, no attach, no emission, one
//! depth count per open and close ([`CommentAttach::skip_depth`]). A skipped subtree
//! claims nothing, so `c` stays the front throughout and every comment behind it starts
//! later still: the one test at the subtree's root covers all of it. The subtree rooted at
//! the opening node `N`, under its parent `P`, is skipped when the queue is empty, or when
//! both of these hold:
//!
//! - `N.end < floor(c)`, where `floor(c)` is where the run of trailing-gap bytes (`,` `)`
//!   space tab) that ends at `c.start` begins — so `c` starts past `N`, and the gap from any
//!   position at or before `N.end` up to `c` holds a byte outside the class;
//! - `N` is not `P`'s last body entry, or `c.start >= P.end`.
//!
//! Rule by rule, for every node `M` in the subtree, `N` included: a leading claim needs
//! `c.start < M.start`, but `M.start <= M.end <= N.end < c.start`; the single trailing claim
//! needs `M.end..c.start` to be all trailing-gap bytes, and that gap holds `floor(c) - 1`; a
//! last-in-body run stops at the parent's end, which inside the subtree is at most `N.end`
//! and for `N` itself is `P.end <= c.start` by the second condition. (The `node.end ==
//! parent.end` suppression only ever withholds a claim.) The root fallback and the
//! preceding-HTML entry are root-only, and the root is never skipped.
//!
//! ⚠️ The proof leans on one fact about the writer: **a non-root node's children end no
//! later than it does** (a child may START earlier — a decorator ahead of `export` — which
//! the proof never needs). The one node that breaks it is a typed Svelte block binding
//! (`{#each xs as a: T}`), whose span is the bare binding while its `typeAnnotation` child
//! runs past it, and that node is always its island's root. A debug build asserts the fact
//! at every open, asserts where the writer emits that node that it opens as its island's
//! root (`CommentAttach::debug_assert_opens_island_root`), and asserts at every skipped
//! node that the rules would have claimed nothing there.

use std::cell::{Cell, RefCell};

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
    /// `floor(c)` for the queue front, keyed by the `head` it was computed for — it
    /// changes only when a claim moves the front, so one backward scan serves every
    /// skip test until then.
    front_floor: Option<(usize, u32)>,
    /// Debug-only: the skipped subtree's open nodes, so a skipped close is still paired
    /// against its open and still checked against the rules it skips.
    #[cfg(debug_assertions)]
    skipped: Vec<SkippedFrame>,
    /// Debug-only: a last-body mark made inside a skipped subtree, which the attach does
    /// not record ([`CommentAttach::mark_last_body_element`]). The next skipped open
    /// consumes it, and a skipped close that finds it still set has caught a marked
    /// element that emitted no node — the check `close_attached` makes of a recorded
    /// mark.
    #[cfg(debug_assertions)]
    skipped_mark_pending: bool,
}

/// Debug-only: one open node of a skipped subtree (see [`State::skipped`]).
#[cfg(debug_assertions)]
struct SkippedFrame {
    node_type: &'static str,
    span: Span,
    /// Known for the subtree's root, whose open ran the attach; `None` below it, where
    /// the attach does not record the last-body mark (see
    /// [`CommentAttach::mark_last_body_element`]) and the check reads both answers.
    is_last_in_body: Option<bool>,
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
    /// How many nodes of a skipped subtree are open (the module doc's skip): `0` while
    /// the attach runs, and while it is not, every open adds one and every close takes
    /// one away with no frame and no emission. A `Cell` beside the `RefCell` rather than
    /// a field inside it, so a skipped open or close touches this one word and nothing
    /// else — no borrow, and none of the attach body's frame.
    skip_depth: Cell<u32>,
    state: RefCell<State>,
}

impl<'a> CommentAttach<'a> {
    /// One island's attach, over the window its caller already settled.
    #[must_use]
    pub fn new(source: &'a str, island: IslandComments<'a>) -> Self {
        Self {
            source,
            island,
            skip_depth: Cell::new(0),
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

    /// A node opens: inside a skipped subtree (the module doc's skip), count it and
    /// nothing else; otherwise run the attach ([`open_attached`](Self::open_attached)).
    ///
    /// `#[cold]` because an ordinary (comment-free) emission never reaches this: keeping
    /// it out of the node emitters' way is what makes `CommentMode` cost one never-taken
    /// compare per node. The skipped path is split from the attach body so that it pays
    /// for neither the `RefCell` borrow nor the body's frame; it is most of an island's
    /// opens.
    ///
    /// `#[inline]` as well, unlike [`close_and_emit`](Self::close_and_emit): its handful
    /// of callers (`node_header_impl` and the hand-written headers) take a skipped open
    /// with no call at all, and the `#[cold]` still marks the branch to it unlikely there.
    #[cold]
    #[inline]
    pub(super) fn open(&self, node_type: &'static str, span: Span) {
        let depth = self.skip_depth.get();
        if depth > 0 {
            self.skip_depth.set(depth + 1);
            #[cfg(debug_assertions)]
            self.debug_open_skipped(node_type, span);
            return;
        }
        self.open_attached(node_type, span);
    }

    /// A node opens outside a skipped subtree: shift every comment before the node's
    /// start onto it as leading — or, when nothing in the subtree it roots can take a
    /// comment, start skipping that subtree.
    // `node_type` feeds only the open/close pairing and the skip's checks, which are
    // debug-only.
    #[cold]
    #[cfg_attr(not(debug_assertions), expect(unused_variables))]
    fn open_attached(&self, node_type: &'static str, span: Span) {
        let st = &mut *self.state.borrow_mut();
        let is_last_in_body = std::mem::take(&mut st.next_is_last_body);
        #[cfg(debug_assertions)]
        self.debug_check_open(st, node_type, span);
        if let Some(parent_end) = st.frames.last().map(|f| f.span.end)
            && self.subtree_takes_nothing(st, span, parent_end, is_last_in_body)
        {
            self.skip_depth.set(1);
            #[cfg(debug_assertions)]
            st.skipped.push(SkippedFrame {
                node_type,
                span,
                is_last_in_body: Some(is_last_in_body),
            });
            return;
        }
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
            is_last_in_body,
        });
    }

    /// Whether nothing in the subtree about to open at `span` can take a comment —
    /// the module doc's skip test, asked of a non-root node whose parent ends at
    /// `parent_end`.
    fn subtree_takes_nothing(
        &self,
        st: &mut State,
        span: Span,
        parent_end: u32,
        is_last_in_body: bool,
    ) -> bool {
        let Some(front) = self.front(st) else {
            return true;
        };
        span.end < self.front_floor(st, front)
            && (!is_last_in_body || front.span.start >= parent_end)
    }

    /// `floor(c)` for the queue front `front`: the start of the run of trailing-gap
    /// bytes that ends where it starts. Cached per front ([`State::front_floor`]).
    fn front_floor(&self, st: &mut State, front: &Comment) -> u32 {
        if let Some((head, floor)) = st.front_floor
            && head == st.head
        {
            return floor;
        }
        let before = &self.source.as_bytes()[..front.span.start as usize];
        let floor = before
            .iter()
            .rposition(|&b| !is_trailing_gap_byte(b))
            .map_or(0, |i| i + 1) as u32;
        st.front_floor = Some((st.head, floor));
        floor
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
    ///
    /// Inside a skipped subtree the mark is not recorded: the element it names opens
    /// inside that subtree too, and a skipped open reads nothing — so no mark is ever
    /// recorded while the attach skips, which is what lets a skipped open leave the
    /// state untouched. A debug build still notes it (`State::skipped_mark_pending`),
    /// so a marked element that emits no node is caught inside a skipped subtree as it
    /// is outside one.
    #[cold]
    pub(super) fn mark_last_body_element(&self) {
        #[cfg(debug_assertions)]
        self.debug_mark_skipped();
        if self.skip_depth.get() == 0 {
            self.state.borrow_mut().next_is_last_body = true;
        }
    }

    /// A node closes: inside a skipped subtree, count it and nothing else; otherwise
    /// decide its trailing comments and emit both lists
    /// ([`close_attached`](Self::close_attached)).
    ///
    /// `#[cold]`, and split from its body, as [`open`](Self::open) — but kept out of
    /// line with `#[inline(never)]`: a body this small is otherwise inlined despite
    /// `#[cold]`, and at the hundred-odd node emitters that close through it that
    /// reshapes the comment-free path it exists to stay out of.
    #[cold]
    #[inline(never)]
    pub(super) fn close_and_emit(
        &self,
        w: &mut JsonWriter,
        node_type: &'static str,
        span: Span,
        loc: LocationMapper<'_>,
    ) {
        let depth = self.skip_depth.get();
        if depth > 0 {
            self.skip_depth.set(depth - 1);
            #[cfg(debug_assertions)]
            self.debug_close_skipped(node_type, span);
            return;
        }
        self.close_attached(w, node_type, span, loc);
    }

    /// A node closes outside a skipped subtree: decide its trailing comments, then emit
    /// both lists.
    #[cold]
    fn close_attached(
        &self,
        w: &mut JsonWriter,
        node_type: &'static str,
        span: Span,
        loc: LocationMapper<'_>,
    ) {
        let st = &mut *self.state.borrow_mut();
        debug_assert!(
            !st.next_is_last_body,
            "{node_type} closed with an unconsumed last-body mark — the marked element \
             emitted no node, so the mark would land on an unrelated one"
        );
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

    /// acorn's post-recursion trailing rule for one node: claim the comments
    /// [`trailing_claim`](Self::trailing_claim) counts off the queue front.
    fn attach_trailing(
        &self,
        st: &mut State,
        span: Span,
        parent_end: Option<u32>,
        is_last_in_body: bool,
    ) {
        // A `push` per claim, never `extend` over the index range: `extend` reserves
        // through `RawVecInner::reserve`, and a `Vec<u32>` call there is the one caller
        // in this crate whose element size is not 1 — enough to stop LLVM specializing
        // every byte-buffer reserve site in the codegen unit (~10 KB of `.text`).
        for _ in 0..self.trailing_claim(st, span, parent_end, is_last_in_body) {
            st.attached.push(st.head as u32);
            st.head += 1;
        }
    }

    /// How many comments, from the queue front, acorn's trailing rule assigns to a
    /// node closing at `span` — the rule itself, apart from applying it, so the skip's
    /// debug check reads the same rule the attach does.
    fn trailing_claim(
        &self,
        st: &State,
        span: Span,
        parent_end: Option<u32>,
        is_last_in_body: bool,
    ) -> usize {
        let Some(first) = self.front(st) else {
            return 0;
        };
        // `if (parent === undefined || node.end !== parent.end)` — a node ending where
        // its parent ends leaves the claim to the parent.
        if parent_end == Some(span.end) {
            return 0;
        }
        if is_last_in_body {
            // Last in a body: several trailing comments, newlines allowed between them,
            // stopping at the parent's own end.
            let rest = &self.island.queue[st.head..];
            match parent_end {
                Some(pe) => rest.iter().take_while(|(c, _)| c.span.start < pe).count(),
                None => rest.len(),
            }
        } else if span.end <= first.span.start {
            // Otherwise at most ONE, and only across a `/^[,) \t]*$/` gap.
            let gap = &self.source.as_bytes()[span.end as usize..first.span.start as usize];
            usize::from(gap.iter().all(|&b| is_trailing_gap_byte(b)))
        } else {
            0
        }
    }

    /// Debug-only, at every open: the fact the skip's proof leans on — a non-root
    /// node's children end no later than it does — and, inside a skipped subtree, that
    /// the leading rule would have claimed nothing here.
    #[cfg(debug_assertions)]
    fn debug_check_open(&self, st: &State, node_type: &'static str, span: Span) {
        let parent = match st.skipped.last() {
            Some(skipped) => Some((skipped.node_type, skipped.span)),
            // The root's children are exempt: a typed block binding's annotation runs
            // past its bare root (the writer asserts that binding is a root where it
            // emits one), and the root is never skipped.
            None if st.frames.len() >= 2 => st.frames.last().map(|f| (f.node_type, f.span)),
            None => None,
        };
        if let Some((parent_type, parent_span)) = parent {
            assert!(
                span.end <= parent_span.end,
                "{node_type} ({},{}) ends past its non-root parent {parent_type} ({},{}) — \
                 the comment attach's subtree skip assumes it cannot",
                span.start,
                span.end,
                parent_span.start,
                parent_span.end,
            );
        }
        if self.skip_depth.get() > 0 {
            assert!(
                self.front(st).is_none_or(|c| c.span.start >= span.start),
                "skipped {node_type} ({},{}) would have taken a leading comment",
                span.start,
                span.end,
            );
        }
    }

    /// Debug-only, ahead of an open: the node about to open at `span` is its island's
    /// root — no node is open, skipped or not. Asked by the writers that emit the
    /// one node whose child may end past it (a typed Svelte block binding — the module
    /// doc's ⚠️), so the skip's assumption is checked where that shape is made.
    #[cfg(debug_assertions)]
    pub(super) fn debug_assert_opens_island_root(&self, node_type: &'static str, span: Span) {
        assert!(
            self.skip_depth.get() == 0 && self.state.borrow().frames.is_empty(),
            "{node_type} ({},{}) has a child ending past it but is not its island's root — \
             the comment attach's subtree skip assumes only a root can",
            span.start,
            span.end,
        );
    }

    /// Debug-only, at a last-body mark: inside a skipped subtree, note the mark the
    /// attach does not record ([`State::skipped_mark_pending`]).
    #[cfg(debug_assertions)]
    fn debug_mark_skipped(&self) {
        if self.skip_depth.get() > 0 {
            self.state.borrow_mut().skipped_mark_pending = true;
        }
    }

    /// Debug-only, at a skipped open: the checks every open takes, then the shadow
    /// frame its close is paired against. The open consumes a pending skipped mark —
    /// the marked element is the node opening here.
    #[cfg(debug_assertions)]
    fn debug_open_skipped(&self, node_type: &'static str, span: Span) {
        let st = &mut *self.state.borrow_mut();
        assert!(
            !st.next_is_last_body,
            "a last-body mark is recorded inside a skipped subtree"
        );
        st.skipped_mark_pending = false;
        self.debug_check_open(st, node_type, span);
        st.skipped.push(SkippedFrame {
            node_type,
            span,
            is_last_in_body: None,
        });
    }

    /// Debug-only, at a skipped close: no last-body mark is left pending, the close
    /// pairs against its open, and the trailing rule would have claimed nothing here.
    #[cfg(debug_assertions)]
    fn debug_close_skipped(&self, node_type: &'static str, span: Span) {
        let st = &mut *self.state.borrow_mut();
        assert!(
            !st.next_is_last_body && !st.skipped_mark_pending,
            "{node_type} closed with an unconsumed last-body mark in a skipped subtree — \
             the marked element emitted no node, so outside a skip the mark would land on \
             an unrelated one"
        );
        let Some(frame) = st.skipped.pop() else {
            debug_assert!(false, "a skipped close without a skipped open: {node_type}");
            return;
        };
        assert!(
            frame.node_type == node_type && frame.span == span,
            "attach open/close mismatch in a skipped subtree: opened {} ({},{}), closed \
             {node_type} ({},{})",
            frame.node_type,
            frame.span.start,
            frame.span.end,
            span.start,
            span.end,
        );
        let Some(parent_end) = st
            .skipped
            .last()
            .map(|parent| parent.span.end)
            .or_else(|| st.frames.last().map(|f| f.span.end))
        else {
            debug_assert!(false, "a skipped subtree's root is never the island root");
            return;
        };
        // Below the subtree's root the mark is not recorded, so both answers are read:
        // the proof makes each claim nothing there.
        let readings: &[bool] = match frame.is_last_in_body {
            Some(is_last_in_body) => &[is_last_in_body][..],
            None => &[false, true],
        };
        for &is_last_in_body in readings {
            assert_eq!(
                self.trailing_claim(st, span, Some(parent_end), is_last_in_body),
                0,
                "skipped {node_type} ({},{}) would have taken a trailing comment \
                 (last in body: {is_last_in_body})",
                span.start,
                span.end,
            );
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
            w.start_end_field(loc.pos(comment.span.start), loc.pos(comment.span.end));
            w.raw("}");
        }
    }
}

/// The byte class of acorn's single-trailing-comment gap, `/^[,) \t]*$/` — one
/// definition because the skip's `floor(c)` is only sound over exactly the class the
/// trailing rule tests.
#[inline]
fn is_trailing_gap_byte(b: u8) -> bool {
    matches!(b, b',' | b')' | b' ' | b'\t')
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
        let skipped = self.skip_depth.get() as usize;
        assert!(
            st.frames.is_empty() && st.attached.is_empty() && skipped == 0,
            "an island finished with {} unclosed node(s) ({skipped} of them skipped) and {} \
             unemitted comment(s)",
            st.frames.len() + skipped,
            st.attached.len(),
        );
        debug_assert_eq!(st.skipped.len(), skipped);
    }
}
