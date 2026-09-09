//! The line-suffix run: draining the pending `line_suffix` buffer onto one line.
//!
//! A flush is a comment **run** — everything the buffer holds lands back-to-back on one
//! physical line — so it owes the separator every other run in the printer owes
//! (`docs/comments.md` §Trailing and dangling runs). A `//` reaches end-of-line, so a
//! suffix landing behind one is welded into it (`x; // c1 // c2` reparses as a single
//! comment whose text happens to contain the second): [`push_run_separator`] breaks
//! first, at the indent the suffix was queued at.
//!
//! The `//` case is deliberately the separator's WHOLE scope. A follower gluing onto a
//! member that merely BROKE (an own-line block's payload) is lossless, and whether the
//! glued pair survives the reparse depends on where it lands — a statement gap's
//! leading run KEEPS a glued pair, the dangling `}`→keyword gap SPLITS it — a fact only
//! the builder that owns the gap has, so a renderer-side break here turned a stable
//! prettier match (the mid-list pair) into a divergence. Builders whose gap splits
//! emit each member with its break inside the suffix instead
//! (`GapCommentRun::Dangling`'s `defer_inline` arm in `tsv_ts`).
//!
//! This is the renderer-level FLOOR under the build-time askers in `tsv_ts`
//! (`Printer::trailer_follows_through_closers` and friends), which keep a comment where
//! the author wrote it but read the *source* and so cannot see a trailer past a further
//! token. Whether two deferred comments share an output line is a layout fact only this
//! module has.

use super::arena::{ArenaCommand, DocId, DocNode, LineSuffixBuf};
use super::arena_render::{RenderCtx, render_line_break, render_single_doc_inner};
use super::types::{LineKind, Mode, resolve_text};

/// Flush pending line suffix content, in the order it was queued. A separator taken by
/// [`push_run_separator`] breaks at the **suffix's own queued indent** — the indent the
/// comment was captured at, which is where a reformat then reads it.
///
/// ⚠️ **The alternative — the indent of the break the flush is happening at — is what this
/// used to do, and the difference is IDEMPOTENCY.** That break belongs to whatever construct
/// is closing (a `)`, a `}`, a `>`), which can sit a level out from where the comment lives,
/// so the second comment of a run landed at an indent the next pass moved. Re-measured over
/// 2,520 targeted two-suffix runs (a gap comment inside a construct plus a statement
/// trailer, across twelve statement hosts — block, method, object property, arrow body,
/// `if` / `for` / `try` bodies, both switch-case positions — and eight type hosts): the
/// closing-break indent leaves **22** non-idempotent, the queued indent **2**, and the
/// queued indent regresses none of them. The switch's LAST case, once this rule's known
/// counterexample, is among the shapes it now settles; the builder-side answer that was
/// added for it (the case's last-statement `;`-line comments defer own-line, dedented to the
/// case's level — `tsv_ts` `statements/control_flow/switch.rs`) stands on its own and still
/// keeps that shape away from this separator. Prettier's nearest construction agrees:
/// `printTrailingComment`'s own-line arm is `lineSuffix([hardline, …])`, breaking at the
/// queued indent — chosen by the *builder*, which meant the indent it captured. Cataloged in
/// `tests/fixtures/typescript/syntax/comments/deferred_comment_run_separator_prettier_divergence`.
///
/// Prettier flushes by re-pushing the buffer onto its command *stack*
/// (`commands.push(line, ...lineSuffix.reverse())`, `printer.js`) — the `reverse()`
/// there exists only to cancel the stack's LIFO pop, so the net emission order is
/// FIFO. This renderer drives the suffixes directly, so it must iterate forward:
/// reversing here would emit two suffixes queued on one line back-to-front. Prettier's
/// flush is that push and nothing else — it has **no separator here at all**, which is why
/// it welds; the sequencing rule it does have (`printTrailingComment` threading
/// `previousComment.hasLineSuffix`) is per-gap, and `push_trailing_comments_in_range`
/// already mirrors it. The hole is strictly ACROSS gaps, and only the renderer can see it:
/// the same source welds or does not depending on print width (`foo(fn() // c⏎.bar, x); //
/// c1` collapses and welds; widen the second argument and the call expands and it cannot).
pub(super) fn flush_line_suffix(
    ctx: &RenderCtx<'_>,
    line_suffix: &mut LineSuffixBuf,
    output: &mut String,
    pos: &mut usize,
    should_remeasure: &mut bool,
) {
    if line_suffix.is_empty() {
        return;
    }
    // A flush of two or more suffixes is a comment RUN, and this loop is its only
    // separator: whatever the buffer holds lands back-to-back on one physical line. A
    // `//` runs to end-of-line, so a suffix behind one is welded into it
    // (`x; // c1 // c2` reparses as ONE comment) — see `push_run_separator`. A lone
    // suffix poses no separator question, and skips the walk (and these borrows) entirely.
    let queued = line_suffix.len();
    let mut pending_line_comment = false;
    for (i, suffix_cmd) in std::mem::take(line_suffix).into_iter().enumerate() {
        let separated = pending_line_comment && push_run_separator(ctx, suffix_cmd, output, pos);
        let content_start = output.len();
        render_single_doc_inner(
            ctx,
            suffix_cmd.doc,
            output,
            pos,
            suffix_cmd.indent(),
            suffix_cmd.mode(),
            None,
            should_remeasure,
        );
        if separated {
            drop_line_head_space(output, pos, content_start);
        }
        // Only a suffix with a successor poses the question, so a lone one — the common
        // case — never walks a doc at all.
        pending_line_comment = i + 1 < queued
            && with_doc_store(ctx, |docs| {
                docs.ends_in_line_comment(suffix_cmd.doc, suffix_cmd.mode())
            });
    }
}

/// The run separator [`flush_line_suffix`] owes a suffix landing behind a `//`: a hard
/// line, unless the suffix already opens its own line (`build_trailing_comment_doc_own_line`
/// carries the break *inside* the suffix, and a second one would fabricate a blank).
///
/// The break takes the **suffix's own** indent, not the flush's: the comment is being placed
/// where it was queued, inside whatever construct captured it, and that is the indent a
/// reformat reads it back at ([`flush_line_suffix`] carries the measurement).
///
/// Deliberately not a `render_line_node` call (private to `arena_render`): the pending
/// buffer was taken by the flush loop, so there is nothing left to flush, and this break is
/// emitted mid-run rather than ending the construct — `should_remeasure` is the enclosing
/// loop's obligation, untouched here.
fn push_run_separator(
    ctx: &RenderCtx<'_>,
    suffix_cmd: ArenaCommand,
    output: &mut String,
    pos: &mut usize,
) -> bool {
    if with_doc_store(ctx, |docs| {
        docs.opens_its_own_line(suffix_cmd.doc, suffix_cmd.mode())
    }) {
        return false;
    }
    render_line_break(
        LineKind::Hard,
        suffix_cmd.mode(),
        suffix_cmd.indent(),
        output,
        pos,
        ctx.render,
        ctx.embed,
    )
}

/// Drop the space a broken-apart suffix left at the head of its new line.
///
/// A trailing-comment suffix carries its own separator as a literal `text(" ")`
/// (`build_trailing_comment_doc`), which is the FLAT spelling of the separator — the same
/// role [`LineKind::Normal`] plays, a space inline and a newline when the run breaks. When
/// [`push_run_separator`] takes the break, that space would print again at the head of the
/// line, so exactly one leading space is consumed by the break that replaced it.
///
/// `pos` follows only when the suffix stayed on that one line: a suffix that broke again
/// internally ends at a column the dropped space never contributed to.
fn drop_line_head_space(output: &mut String, pos: &mut usize, content_start: usize) {
    if output.as_bytes().get(content_start) != Some(&b' ') {
        return;
    }
    output.remove(content_start);
    if !output[content_start..].contains('\n') {
        *pos -= 1;
    }
}

/// Which end of a doc [`DocStore::edge_emission`] inspects.
#[derive(Clone, Copy)]
enum Edge {
    First,
    Last,
}

/// What the renderer emits at one [`Edge`] of a doc — the shared answer behind both
/// questions the line-suffix run separator asks ("does this leave a `//` running to end of
/// line?" and "does this open its own line?"), so one node-kind table serves both.
enum EdgeEmission<'a> {
    /// Literal text, resolved.
    Text(&'a str),
    /// A real line break.
    Break,
    /// Emits text this walk does not resolve to a slice — a [`DocNode::MultilineText`]
    /// body, whose ends are its own `/*` and `*/`. Neither question this serves can be
    /// `true` for it, and the walk stops rather than reading past to some earlier node.
    Opaque,
}

/// The arena storage a suffix-run walk reads.
struct DocStore<'a> {
    nodes: &'a [DocNode],
    children: &'a [DocId],
    pool: &'a str,
    source: Option<&'a str>,
}

/// Run `f` over the arena's doc storage.
///
/// Scoped to the call rather than hoisted across [`flush_line_suffix`]'s loop on purpose:
/// the loop renders each suffix through `render_single_doc_inner`, and an arena borrow held
/// across that would make any future allocation there a panic instead of a compile error.
/// The walks are a cold path — reached only by a suffix that has a successor — so the three
/// borrows cost nothing worth that coupling.
#[inline]
fn with_doc_store<R>(ctx: &RenderCtx<'_>, f: impl FnOnce(&DocStore<'_>) -> R) -> R {
    let nodes = ctx.arena.borrow_nodes();
    let children = ctx.arena.borrow_children();
    let pool = ctx.arena.borrow_text_pool();
    f(&DocStore {
        nodes: &nodes,
        children: &children,
        pool: &pool,
        source: ctx.source,
    })
}

impl<'a> DocStore<'a> {
    /// What `doc` emits at `edge`, or `None` when it emits nothing at all (an empty text,
    /// a collapsed soft line, a pure layout marker) — for which the caller's walk keeps
    /// going toward that edge's neighbour.
    fn edge_emission(&self, doc: DocId, mode: Mode, edge: Edge) -> Option<EdgeEmission<'a>> {
        match &self.nodes[doc.index()] {
            DocNode::Text(t) => match resolve_text(t, self.source, self.pool) {
                "" => None,
                s => Some(EdgeEmission::Text(s)),
            },
            // Flat, a collapsible line is the space (or nothing) it stands for; broken —
            // and always, for a hard one — it is a real break.
            DocNode::Line(kind) => match (kind, mode) {
                (LineKind::Hard | LineKind::Literal, _) | (_, Mode::Break) => {
                    Some(EdgeEmission::Break)
                }
                (LineKind::Normal, Mode::Flat) => Some(EdgeEmission::Text(" ")),
                (LineKind::Soft, Mode::Flat) => None,
            },
            DocNode::MultilineText { .. } => Some(EdgeEmission::Opaque),
            // Resolved exactly as the flush's own sub-render resolves it: that render
            // carries no group-mode map, so a keyed conditional reads its group as
            // unresolved → flat, and an unkeyed one keys on the mode it is rendered in.
            DocNode::IfBreak {
                break_doc,
                flat_doc,
                group_id,
            } => {
                let broke = group_id.is_none() && mode == Mode::Break;
                self.edge_emission(if broke { *break_doc } else { *flat_doc }, mode, edge)
            }
            DocNode::Indent(inner) | DocNode::Dedent(inner) | DocNode::LineSuffix(inner) => {
                self.edge_emission(*inner, mode, edge)
            }
            DocNode::AlignRoot { contents, .. }
            | DocNode::Align { contents, .. }
            | DocNode::IndentIfBreak { contents, .. }
            | DocNode::Group { contents, .. }
            | DocNode::GatedState { contents, .. } => self.edge_emission(*contents, mode, edge),
            DocNode::WithContext { doc, .. } => self.edge_emission(*doc, mode, edge),
            DocNode::Concat(range) | DocNode::Fill(range) => {
                let children = range.resolve(self.children);
                let mut walk = children.iter();
                let step = |child: &DocId| self.edge_emission(*child, mode, edge);
                match edge {
                    Edge::First => walk.find_map(step),
                    Edge::Last => walk.rev().find_map(step),
                }
            }
            // Pure layout markers: nothing reaches the output buffer.
            DocNode::BreakParent
            | DocNode::FlushBreak
            | DocNode::FlowProbeEnd
            | DocNode::LineSuffixBoundary => None,
        }
    }

    /// Whether rendering `doc` leaves the output line running inside a `//` comment — its
    /// last emitted text is a whole line comment, with no break after it.
    ///
    /// Exact rather than a scan of the emitted bytes: a `line_suffix` payload is comment
    /// text, and a block comment may legitimately *contain* `//` (a URL), so only the node
    /// identity answers. A line comment reaches the renderer as ONE text node carrying the
    /// whole comment (`DocArena::line_comment_source_span` / `line_comment_text_pooled`),
    /// so the leading `//` of the last emitted text is that identity — the same one-node
    /// spelling `crate::doc::swallow` keys its diagnostic on. The hashbang joins it: both
    /// run to end-of-line.
    fn ends_in_line_comment(&self, doc: DocId, mode: Mode) -> bool {
        matches!(
            self.edge_emission(doc, mode, Edge::Last),
            Some(EdgeEmission::Text(s)) if s.starts_with("//") || s.starts_with("#!")
        )
    }

    /// Whether rendering `doc` starts by ending the current line, so a run separator ahead
    /// of it would fabricate a blank. The mirror of [`Self::ends_in_line_comment`], read
    /// from the other edge.
    fn opens_its_own_line(&self, doc: DocId, mode: Mode) -> bool {
        matches!(
            self.edge_emission(doc, mode, Edge::First),
            Some(EdgeEmission::Break)
        )
    }
}
