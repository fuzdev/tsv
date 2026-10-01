//! The flat write-out: a doc that fits the rest of its line, written in one walk.
//!
//! [`arena_try_print_flat_into`] stands in for
//! [`arena_print_doc_with_indent_resolved_preserve_whitespace_into`](super::arena_print_doc_with_indent_resolved_preserve_whitespace_into)
//! on the docs that entry would print on one line, and refuses every other doc, which
//! the caller then hands to the renderer. The module is the argument that the two agree.
//!
//! # The claim
//!
//! *If the walk accepts a doc, the renderer — given the same doc, column, embed
//! context and source — writes exactly the bytes the walk wrote, and leaves no other
//! trace.* One direction only: a doc the renderer would print flat may still be
//! refused (a line suffix that flushes on the same line, an `if_break` whose flat arm
//! renders), and a refusal costs nothing but the fallback.
//!
//! The walk carries `left`, the columns still free: it starts at
//! `print_width − embed.suffix_width − start_column` (saturating) and gives up a text's
//! cached width, one column per flat `line`, and each [`DocContext::trailing_reserve`]
//! it passes. Call what it gives up from some point on the walk's *future charge*. The
//! walk accepts only if `left` never goes negative, so at every point the future charge
//! is at most `left`.
//!
//! The renderer's `pos` at the same point is `start_column` plus the text and line
//! columns alone (a reserve moves no column), and the width its group checks measure
//! against is `print_width − effective_suffix_width(pos) − pos`, where the effective
//! suffix is `embed.suffix_width` or zero. A fill measures against `print_width − pos`,
//! less its own context's reserve on its final segment — a reserve the walk gave up
//! when it entered that context. Either way **the renderer's budget at a point is at
//! least the walk's `left` there.**
//!
//! Every layout decision the renderer takes on an accepted doc is an
//! `arena_fits_with_lookahead` call (or `arena_fits_multi`, a wrapper over it) with no
//! suffix pending. That walk charges the measured doc flat — a text its cached width, a
//! `Normal` line one column, a `Soft` line none, a context its reserve: the walk's own
//! charges, node for node — then the rest commands in document order, and stops with
//! "fits" at the first line it meets in break mode or when the commands run out (a
//! group in the look-ahead inherits its command's mode, so in break mode the measure
//! may end inside it — fewer charges, never more). What it charges is therefore a run
//! of nodes that all lie ahead of the decision point in document order, each at the
//! walk's price: a part of the walk's future charge. So every such call answers "fits",
//! and it remains to read off, kind by kind, what the renderer does when every fits
//! call says yes.
//!
//! # Node kinds
//!
//! "Flat" below is the walk's `flat` flag: set inside a group or a fill, clear at the
//! root, where the renderer's mode is `Break`.
//!
//! - **`Text`** with a cached width: both append the resolved slice, and the renderer
//!   advances `pos` by the same cached width the walk charges. A text holding a
//!   newline (the width slot's newline flag) is refused: it ends the line.
//! - **`Concat`**: children in order, in the enclosing mode.
//! - **`Line`**, flat: the renderer writes a space for `Normal` and nothing for `Soft`,
//!   in a flat group and in a fill alike (below). Not flat, the renderer's mode is
//!   `Break` and the line is a newline: refused. `Hard` and `Literal` break in either
//!   mode: refused.
//! - **`Group`** with no expanded states, not pre-broken, not keyed: the renderer breaks
//!   it without measuring only when `will_break(contents)` — a newline-bearing text, a
//!   `MultilineText`, a hard line, a pre-broken group or a `BreakParent` somewhere
//!   inside, each of which the walk refuses on reaching it. Otherwise it is either
//!   passed through flat (already in flat mode, nothing to remeasure) or measured, and
//!   the measure fits. Its contents render in flat mode. A conditional group (its
//!   ladder may pick a later state), a pre-broken one and a keyed one (the render
//!   records its mode for readers the walk has no map for) are refused.
//! - **`Fill`**: the fill renderer never reads the enclosing mode. With a default
//!   context, or one that only reserves, its hold and flow arms are off, and each
//!   round asks: does the content fit (alone, or with the rest commands on the final
//!   segment); for a collapsible line in a content slot, does it fit together with the
//!   part after it; on the last pair, does the separator fit ahead of the rest; and
//!   otherwise do content, separator and next content fit together. Each is a fits call
//!   of the shape above, so each says yes, and on yes every arm renders its content and
//!   its separator through the single-doc sub-render **in flat mode** — the same loop
//!   as the top level, where a nested group passes through or measures (and fits) and a
//!   nested fill repeats this paragraph with the sub-render's commands as its rest. No
//!   arm reached on yes writes a newline or trims. So a fill is its parts, flat, in
//!   order: the walk sets `flat` and continues as for a concat.
//! - **`Indent`, `Dedent`, `Align`, `AlignRoot`**: these change the indent a later
//!   line break would write. An accepted doc breaks no line.
//! - **`WithContext`** carrying no layout flag: the renderer descends (handing the
//!   context to a fill directly under it, read above); the fits walk charges the
//!   reserve, and so does this one. Any flag — the Svelte flow and glue policies, the
//!   flow probe and its hold — is refused.
//! - **Everything else is refused**: `MultilineText`, `IfBreak`, `IndentIfBreak`,
//!   `LineSuffix`, `LineSuffixBoundary`, `EmbedEnd`, `BreakParent`, `FlushBreak`,
//!   `FlowProbeEnd`, `GatedState`. Several render to nothing in some states; none is
//!   worth an argument here.
//!
//! # What the renderer leaves besides bytes
//!
//! - **No trim.** The renderer trims trailing whitespace before a line break, which an
//!   accepted doc never reaches, and once more at the end on every entry but the
//!   preserve-whitespace one. That is why this stands in for that entry alone: a flat
//!   doc may end in a space (a trailing flat `line`, a CSS escape's payload) which the
//!   other entries would strip. It is also why the walk may append to a buffer that
//!   already holds text, where the renderer needs an empty one: nothing here reaches
//!   backwards.
//! - **Keyed groups and flow probes.** The renderer clears the arena's keyed-group map
//!   on entry and records keyed groups and embed ends into it; the map is read only
//!   while a render is running — by `IfBreak`, `IndentIfBreak`, the embed-end shed and
//!   the line-suffix flush's edge classifier — and the next render clears it first. The
//!   walk refuses every writer and every node those readers start from (the classifier
//!   runs only on a pending line suffix), so a map left uncleared is never read. Flow
//!   probes are opened and read only under context flags, all refused.
//! - **The layout memo** the fits walks fill is a cache of pure functions of the doc.
//! - **The audit seams** (`swallow_check`, `comment_check`): the renderer notes a
//!   line-comment text and what follows it on the line, and records a tagged comment
//!   node as emitted. The walk makes no such note, so it refuses a doc holding either
//!   kind of node while its check is armed: an accepted doc is one the renderer would
//!   have noted nothing for, beyond clearing the pending line comment a render starts
//!   by clearing — which the walk does too. For the one caller this refusal never
//!   fires: the CSS printer builds neither kind of node (a comment in its docs is
//!   plain text, and it feeds the ledger where it writes), so in an audit build its
//!   comment-bearing docs take the flat path as they do in release, and what grades
//!   them there is the twin. The refusal is the contract for a caller whose docs do
//!   carry those nodes.
//!
//! # The twin
//!
//! Debug builds, and any build carrying an audit seam (so the `corpus`-profile audit
//! binaries, which are release builds), render every accepted doc through the renderer
//! as well and assert the bytes and the final column agree.

use super::arena::{DocArena, DocId, DocNode};
#[cfg(any(debug_assertions, feature = "swallow_check", feature = "comment_check"))]
use super::arena_render::{RenderCtx, render_doc_iterative};
use super::render_config::RenderConfig;
use super::specialize_short_len;
#[cfg(feature = "swallow_check")]
use super::swallow::SwallowTracker;
#[cfg(doc)]
use super::types::DocContext;
use super::types::{CachedWidth, DocText, LineKind, resolve_text};
use crate::EmbedContext;
#[cfg(feature = "comment_check")]
use crate::comment_ledger;

/// How deep the walk recurses before it refuses. A doc nested past this is rare and
/// wide; the bound keeps the walk's stack use a constant.
const MAX_DEPTH: u32 = 48;

/// Write `doc` flat onto the end of `output` if it fits the rest of the current line,
/// returning whether it did. On `false` the buffer is exactly as it was, and the caller
/// renders the doc through
/// [`arena_print_doc_with_indent_resolved_preserve_whitespace_into`](super::arena_print_doc_with_indent_resolved_preserve_whitespace_into)
/// with the same arguments; on `true` the buffer holds what that render would have
/// produced, appended. The arguments are that entry's, and `output` need not be empty.
///
/// It replaces that entry only — a trimming entry strips a trailing space this keeps.
/// The module doc carries the argument, node kind by node kind.
#[inline]
pub fn arena_try_print_flat_into(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    source: &str,
    output: &mut String,
) -> bool {
    try_print_flat(
        arena,
        doc,
        &RenderConfig::default(),
        embed,
        start_column,
        start_indent_level,
        source,
        output,
    )
}

/// [`arena_try_print_flat_into`] at an explicit [`RenderConfig`], the seam the unit
/// tests reach smaller print widths through.
#[expect(clippy::too_many_arguments)]
#[inline]
pub(super) fn try_print_flat(
    arena: &DocArena,
    doc: DocId,
    render: &RenderConfig,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    source: &str,
    output: &mut String,
) -> bool {
    // Read by the twin alone: an accepted doc breaks no line, so no indent is written.
    let _ = start_indent_level;
    let budget = render
        .print_width
        .saturating_sub(embed.suffix_width)
        .saturating_sub(start_column);
    let mark = output.len();
    let mut left = budget as isize;
    #[cfg(any(debug_assertions, feature = "swallow_check", feature = "comment_check"))]
    let reserved;
    let wrote = {
        let nodes = arena.borrow_nodes();
        let children = arena.borrow_children();
        let pool = arena.borrow_text_pool();
        let walk = FlatWalk {
            nodes: &nodes,
            children: &children,
            pool: &pool,
            source,
            #[cfg(any(feature = "swallow_check", feature = "comment_check"))]
            arena,
            // A render starts by clearing the line comment left pending by the one
            // before it, and this stands for a render.
            #[cfg(feature = "swallow_check")]
            swallow_on: SwallowTracker::begin_render().enabled(),
            // The renderer's own gate, at the purpose every output entry renders with.
            #[cfg(feature = "comment_check")]
            ledger_on: comment_ledger::comment_check_enabled() && arena.has_comment_docs(),
            #[cfg(any(debug_assertions, feature = "swallow_check", feature = "comment_check"))]
            reserved: std::cell::Cell::new(0),
        };
        let wrote = walk.write(doc, false, output, &mut left, 0);
        #[cfg(any(debug_assertions, feature = "swallow_check", feature = "comment_check"))]
        {
            reserved = walk.reserved.get();
        }
        wrote
    };
    if !wrote {
        output.truncate(mark);
        return false;
    }

    #[cfg(any(debug_assertions, feature = "swallow_check", feature = "comment_check"))]
    {
        let mut rendered = String::new();
        let mut pos = start_column;
        render_doc_iterative(
            &RenderCtx {
                arena,
                render,
                embed,
                source: Some(source),
            },
            doc,
            &mut rendered,
            &mut pos,
            start_indent_level,
        );
        assert_eq!(
            &output[mark..],
            rendered,
            "a doc written flat rendered as something else"
        );
        // `budget − left` is everything the walk gave up; less the reserves, it is the
        // columns the written text occupies.
        assert_eq!(
            start_column + (budget - left.unsigned_abs()) - reserved,
            pos,
            "a doc written flat left the renderer at another column"
        );
    }
    true
}

/// The invariant context of one flat walk — the arena borrows and the document source,
/// and under the audit features what the two seams key on.
struct FlatWalk<'a> {
    nodes: &'a [DocNode],
    children: &'a [DocId],
    pool: &'a str,
    source: &'a str,
    #[cfg(any(feature = "swallow_check", feature = "comment_check"))]
    arena: &'a DocArena,
    #[cfg(feature = "swallow_check")]
    swallow_on: bool,
    #[cfg(feature = "comment_check")]
    ledger_on: bool,
    /// The trailing reserves the walk has given up, which the twin takes back out of
    /// the charge to get the column.
    #[cfg(any(debug_assertions, feature = "swallow_check", feature = "comment_check"))]
    reserved: std::cell::Cell<usize>,
}

/// Refuse the doc — the walk's `return false`, with the reason named at the site. The
/// reason is for the reader; nothing reads it at run time.
macro_rules! refuse {
    ($reason:literal) => {
        return false
    };
}

impl FlatWalk<'_> {
    /// Whether the renderer would note `id` to an armed audit seam: a line-comment
    /// text (the swallow check), or a node tagged as a comment's doc (the ledger).
    /// Constant `false` without the features.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    fn renderer_notes(&self, id: DocId) -> bool {
        #[cfg(feature = "swallow_check")]
        if self.swallow_on && self.arena.is_line_comment(id) {
            return true;
        }
        #[cfg(feature = "comment_check")]
        if self.ledger_on && self.arena.comment_doc_tag(id).is_some() {
            return true;
        }
        let _ = id;
        false
    }

    /// Append a text and give up its width; `false` if it holds a newline or no longer
    /// fits. The copy is the render loop's (`render_text`): short lengths as constants.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    fn text(&self, text: &DocText, out: &mut String, left: &mut isize) -> bool {
        let CachedWidth::Width(w) = text.cached_width() else {
            refuse!("newline text");
        };
        *left -= w as isize;
        if *left < 0 {
            refuse!("width");
        }
        let s = resolve_text(text, Some(self.source), self.pool);
        specialize_short_len!(s.len(), [0, 1, 2, 3, 4, 5, 6, 7, 8], out.push_str(s));
        true
    }

    /// Write the subtree at `id`, or return `false` having written part of it — the
    /// caller truncates. `flat` is whether a group or a fill encloses `id`.
    ///
    /// A container's text and line children are answered in its own loop, so the
    /// recursion is one level per nested container, and a node that forwards to a
    /// single child (a group, an indent, a context) continues in place.
    fn write(
        &self,
        mut id: DocId,
        mut flat: bool,
        out: &mut String,
        left: &mut isize,
        depth: u32,
    ) -> bool {
        loop {
            if self.renderer_notes(id) {
                refuse!("audit seam");
            }
            match &self.nodes[id.index()] {
                DocNode::Text(text) => return self.text(text, out, left),
                node @ (DocNode::Concat(range) | DocNode::Fill(range)) => {
                    let flat = flat || matches!(node, DocNode::Fill(_));
                    for &kid in range.resolve(self.children) {
                        if self.renderer_notes(kid) {
                            refuse!("audit seam");
                        }
                        match &self.nodes[kid.index()] {
                            DocNode::Text(text) => {
                                if !self.text(text, out, left) {
                                    return false;
                                }
                            }
                            DocNode::Line(LineKind::Normal) if flat => {
                                *left -= 1;
                                out.push(' ');
                            }
                            DocNode::Line(LineKind::Soft) if flat => {}
                            _ => {
                                if depth == MAX_DEPTH {
                                    refuse!("depth");
                                }
                                if !self.write(kid, flat, out, left, depth + 1) {
                                    return false;
                                }
                            }
                        }
                    }
                    if *left < 0 {
                        refuse!("width");
                    }
                    return true;
                }
                DocNode::Line(kind) => {
                    return match kind {
                        LineKind::Normal if flat => {
                            *left -= 1;
                            out.push(' ');
                            if *left < 0 {
                                refuse!("width");
                            }
                            true
                        }
                        LineKind::Soft if flat => true,
                        LineKind::Normal | LineKind::Soft => refuse!("line outside a group"),
                        LineKind::Hard | LineKind::Literal => refuse!("hard line"),
                    };
                }
                DocNode::Indent(inner) | DocNode::Dedent(inner) => id = *inner,
                DocNode::Align { contents, .. } | DocNode::AlignRoot { contents, .. } => {
                    id = *contents;
                }
                DocNode::Group {
                    contents,
                    expanded_states,
                    keyed,
                    should_break,
                } => {
                    if *should_break {
                        refuse!("pre-broken group");
                    }
                    if !expanded_states.is_empty() {
                        refuse!("conditional group");
                    }
                    if *keyed {
                        refuse!("keyed group");
                    }
                    flat = true;
                    id = *contents;
                }
                DocNode::WithContext { doc, context } => {
                    if context.has_layout_flag() {
                        refuse!("context flag");
                    }
                    let reserve = context.trailing_reserve();
                    *left -= reserve as isize;
                    if *left < 0 {
                        refuse!("width");
                    }
                    #[cfg(any(
                        debug_assertions,
                        feature = "swallow_check",
                        feature = "comment_check"
                    ))]
                    self.reserved
                        .set(self.reserved.get() + usize::from(reserve));
                    id = *doc;
                }
                DocNode::MultilineText { .. } => refuse!("multiline text"),
                DocNode::IfBreak { .. } | DocNode::IndentIfBreak { .. } => refuse!("if_break"),
                DocNode::LineSuffix(_) | DocNode::LineSuffixBoundary | DocNode::EmbedEnd { .. } => {
                    refuse!("line suffix")
                }
                DocNode::BreakParent | DocNode::FlushBreak => refuse!("break parent"),
                DocNode::FlowProbeEnd | DocNode::GatedState { .. } => refuse!("other"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! The flat walk against the renderer.
    //!
    //! **No corpus can grade the width arithmetic here**: an accepted doc that should
    //! have been refused changes the output only when a line lands on the print width
    //! exactly, and every accepted doc is one the renderer prints identically nearly
    //! always. The boundary tests pin each charge at the column where it decides, the
    //! differential test drives generated docs through both paths, and the twin inside
    //! [`try_print_flat`] re-renders every accepted doc these tests write.
    use super::super::arena::{DocArena, DocId};
    use super::super::arena_render::{RenderCtx, render_doc_iterative};
    use super::super::render_config::RenderConfig;
    use super::super::types::DocContext;
    use super::{MAX_DEPTH, try_print_flat};
    use crate::{EmbedContext, Span};

    /// What sits in the buffer ahead of the doc: it ends in the bytes a trim would eat.
    const BASE: &str = "base \t";

    fn width(print_width: usize) -> RenderConfig {
        RenderConfig {
            print_width,
            ..RenderConfig::default()
        }
    }

    fn suffix(suffix_width: usize) -> EmbedContext {
        EmbedContext {
            suffix_width,
            ..EmbedContext::default()
        }
    }

    /// The flat walk's output, or `None` on a refusal — asserting either way that the
    /// text ahead of the doc is untouched, and on a refusal that nothing was left
    /// behind it.
    fn flat(
        a: &DocArena,
        doc: DocId,
        render: &RenderConfig,
        embed: &EmbedContext,
        column: usize,
        source: &str,
    ) -> Option<String> {
        let mut out = String::from(BASE);
        if try_print_flat(a, doc, render, embed, column, 1, source, &mut out) {
            let written = out
                .strip_prefix(BASE)
                .expect("an accepted doc changed the text ahead of it");
            Some(written.to_string())
        } else {
            assert_eq!(out, BASE, "a refused doc left the buffer changed");
            None
        }
    }

    /// The renderer's output through the preserve-whitespace entry's own body.
    fn rendered(
        a: &DocArena,
        doc: DocId,
        render: &RenderConfig,
        embed: &EmbedContext,
        column: usize,
        source: &str,
    ) -> String {
        let mut out = String::new();
        let mut pos = column;
        render_doc_iterative(
            &RenderCtx {
                arena: a,
                render,
                embed,
                source: Some(source),
            },
            doc,
            &mut out,
            &mut pos,
            1,
        );
        out
    }

    /// `doc` charges exactly `cost` columns: at every start column and suffix width it
    /// is accepted when `column + cost + suffix` reaches the print width and refused
    /// one column short of that.
    fn assert_charges(a: &DocArena, doc: DocId, cost: usize, expect: &str) {
        for column in [0usize, 1, 7] {
            for suffix_width in [0usize, 1, 3] {
                let need = column + cost + suffix_width;
                let embed = suffix(suffix_width);
                assert_eq!(
                    flat(a, doc, &width(need), &embed, column, "").as_deref(),
                    Some(expect),
                    "refused at the exact width (column {column}, suffix {suffix_width})"
                );
                assert_eq!(
                    flat(a, doc, &width(need + 5), &embed, column, "").as_deref(),
                    Some(expect),
                );
                if cost > 0 {
                    assert_eq!(
                        flat(a, doc, &width(need - 1), &embed, column, ""),
                        None,
                        "accepted one column over (column {column}, suffix {suffix_width})"
                    );
                }
            }
        }
    }

    #[test]
    fn a_text_charges_its_width() {
        let a = DocArena::new();
        assert_charges(&a, a.text("abcd"), 4, "abcd");
        assert_charges(&a, a.text(""), 0, "");
        // Display width, not bytes: a wide character is two columns, a tab TAB_WIDTH.
        assert_charges(&a, a.text_pooled("中文"), 4, "中文");
        assert_charges(&a, a.text_pooled("é"), 1, "é");
        assert_charges(&a, a.text_pooled("a\tb"), 4, "a\tb");
        let source = "xx héllo yy";
        let span = a.source_span(Span::new(3, 9), source);
        assert_eq!(
            flat(&a, span, &width(5), &suffix(0), 0, source).as_deref(),
            Some("héllo")
        );
        assert_eq!(flat(&a, span, &width(4), &suffix(0), 0, source), None);
    }

    #[test]
    fn a_flat_line_charges_a_column_and_a_soft_line_none() {
        let a = DocArena::new();
        let doc = a.group(a.concat(&[
            a.text("a"),
            a.line(),
            a.text("b"),
            a.softline(),
            a.text("c"),
        ]));
        assert_charges(&a, doc, 4, "a bc");
        // A line that is the group's whole content, and one that ends it: the space is
        // kept, as the preserve-whitespace entry keeps it.
        assert_charges(&a, a.group(a.line()), 1, " ");
        assert_charges(&a, a.group(a.softline()), 0, "");
        assert_charges(&a, a.group(a.concat(&[a.text("a"), a.line()])), 2, "a ");
        // Through the single-child forwards.
        let forwarded = a.group(a.indent(a.dedent(a.align(2, a.align_root(1, a.line())))));
        assert_charges(&a, forwarded, 1, " ");
    }

    #[test]
    fn a_fill_is_flat_without_a_group() {
        let a = DocArena::new();
        let parts = [a.text("a"), a.line(), a.text("bb"), a.line(), a.text("c")];
        assert_charges(&a, a.fill(&parts), 6, "a bb c");
        // Either parity, any length, and lines nested in a part.
        assert_charges(&a, a.fill(&parts[1..]), 5, " bb c");
        assert_charges(&a, a.fill(&parts[..4]), 5, "a bb ");
        assert_charges(&a, a.fill(&parts[..1]), 1, "a");
        assert_charges(&a, a.fill(&parts[1..2]), 1, " ");
        assert_charges(&a, a.fill(&[]), 0, "");
        let nested = a.fill(&[
            a.concat(&[a.text("a"), a.line(), a.text("b")]),
            a.softline(),
            a.fill(&[a.text("c"), a.line(), a.text("d")]),
        ]);
        assert_charges(&a, nested, 6, "a bc d");
    }

    #[test]
    fn a_context_charges_its_reserve() {
        let a = DocArena::new();
        let parts = [a.text("a"), a.line(), a.text("b")];
        let reserving = |doc| a.with_context(doc, DocContext::reserving(3));
        assert_charges(&a, reserving(a.fill(&parts)), 6, "a b");
        assert_charges(&a, reserving(a.group(a.concat(&parts))), 6, "a b");
        assert_charges(&a, reserving(a.text("ab")), 5, "ab");
        // Nested reserves add.
        assert_charges(&a, reserving(reserving(a.text("ab"))), 8, "ab");
        // A reserve inside a fill part, and after the text it follows.
        let inner = a.fill(&[a.text("a"), a.line(), reserving(a.text("b"))]);
        assert_charges(&a, inner, 6, "a b");
    }

    #[test]
    fn a_start_column_past_the_width_leaves_room_for_nothing() {
        let a = DocArena::new();
        let embed = suffix(2);
        assert_eq!(
            flat(&a, a.text(""), &width(10), &embed, 30, "").as_deref(),
            Some("")
        );
        assert_eq!(
            flat(&a, a.group(a.softline()), &width(10), &embed, 30, "").as_deref(),
            Some("")
        );
        assert_eq!(flat(&a, a.text("a"), &width(10), &embed, 30, ""), None);
        assert_eq!(flat(&a, a.group(a.line()), &width(10), &embed, 9, ""), None);
    }

    /// Every refused shape at the front, in the middle and at the end of a group that
    /// would otherwise fit — so each is refused wherever the walk meets it, with text
    /// already written when it does.
    #[test]
    fn each_refused_kind_is_refused_at_every_position() {
        let a = DocArena::new();
        let source = "a\nb";
        let span = Span::new(0, 3);
        let keyed = a.group_with_id(a.text("k"));
        let flagged = [
            DocContext::default().with_break_before_wide_flow(true),
            DocContext::default().with_after_element_fold(true),
            DocContext::default().with_glued_lead(true),
            DocContext::default().with_glued_atom(true),
            DocContext::default().with_flow_break_probe(true),
            DocContext::default().with_hold_line_after_broken_flow(true),
        ];
        let mut refused = vec![
            ("newline text", a.text_pooled("a\nb")),
            ("static newline text", a.text("a\nb")),
            ("source span over a newline", a.source_span(span, source)),
            // The one text kind `will_break` reads as unbroken while its line still ends.
            (
                "verbatim span over a newline",
                a.verbatim_source_span(span, source),
            ),
            ("multiline text", a.multiline_text("a\nb")),
            ("hard line", a.hardline()),
            ("literal line", a.literalline()),
            ("pre-broken group", a.group_break(a.text("g"))),
            (
                "conditional group",
                a.conditional_group(&[a.text("s"), a.text("t")]),
            ),
            ("keyed group", keyed.doc()),
            ("if_break", a.if_break(a.text("b"), a.text("f"))),
            (
                "keyed if_break",
                a.if_break_with_id(a.text("b"), a.text("f"), keyed),
            ),
            ("indent_if_break", a.indent_if_break(a.text("i"), keyed)),
            ("line suffix", a.line_suffix(a.text("s"))),
            ("line suffix boundary", a.line_suffix_boundary()),
            ("embed end", a.embed_end(None, false).doc()),
            ("break parent", a.break_parent()),
            ("flush break", a.flush_break()),
            ("gated state", a.gated_state(a.text("p"), a.text("c"))),
        ];
        for context in flagged {
            refused.push((
                "flagged context",
                a.with_context(a.fill(&[a.text("c")]), context),
            ));
        }
        let render = width(1000);
        let embed = suffix(0);
        for (name, node) in refused {
            let x = a.text("xx");
            let y = a.text("yy");
            for parts in [
                vec![node],
                vec![node, x, y],
                vec![x, node, y],
                vec![x, y, node],
            ] {
                let shapes = [
                    a.concat(&parts),
                    a.group(a.concat(&parts)),
                    a.fill(&parts),
                    a.group(a.indent(a.concat(&[x, a.concat(&parts)]))),
                    a.fill(&[x, a.line(), a.group(a.concat(&parts))]),
                ];
                for doc in shapes {
                    assert_eq!(flat(&a, doc, &render, &embed, 0, source), None, "{name}");
                }
            }
        }
    }

    #[test]
    fn a_line_outside_a_group_or_fill_is_refused() {
        let a = DocArena::new();
        let render = width(1000);
        let embed = suffix(0);
        for line in [a.line(), a.softline()] {
            for doc in [
                line,
                a.concat(&[a.text("a"), line, a.text("b")]),
                a.indent(a.concat(&[a.text("a"), line])),
                a.with_context(a.concat(&[line, a.text("b")]), DocContext::reserving(1)),
                // A group does not make its SIBLING flat.
                a.concat(&[a.group(a.text("a")), line]),
                a.concat(&[a.fill(&[a.text("a"), a.line(), a.text("b")]), line]),
            ] {
                assert_eq!(flat(&a, doc, &render, &embed, 0, ""), None);
                assert!(rendered(&a, doc, &render, &embed, 0, "").contains('\n'));
            }
        }
    }

    #[test]
    fn nesting_past_the_depth_bound_is_refused() {
        let a = DocArena::new();
        // `containers` nested two-part concats: the root, and one recursion per level
        // below it.
        let nest = |containers: u32| {
            let mut doc = a.text("x");
            for _ in 0..containers {
                doc = a.concat(&[doc, a.text("y")]);
            }
            doc
        };
        let render = width(1000);
        let embed = suffix(0);
        let deepest = nest(MAX_DEPTH + 1);
        assert_eq!(
            flat(&a, deepest, &render, &embed, 0, ""),
            Some(format!("x{}", "y".repeat(MAX_DEPTH as usize + 1)))
        );
        assert_eq!(flat(&a, nest(MAX_DEPTH + 2), &render, &embed, 0, ""), None);
        // Single-child forwards do not count: they continue in place.
        let mut forwards = a.text("x");
        for _ in 0..4 * MAX_DEPTH {
            forwards = a.group(a.indent(forwards));
        }
        assert_eq!(
            flat(&a, forwards, &render, &embed, 0, "").as_deref(),
            Some("x")
        );
    }

    /// A small deterministic generator — xorshift64*, seeded per test.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() >> 33) as usize % n
        }
    }

    /// A generated doc with what the walk should make of it: the columns it charges,
    /// and whether it holds only shapes the walk accepts.
    struct Generated {
        doc: DocId,
        cost: usize,
        accepted: bool,
    }

    /// Texts with their display widths, stated by hand: ASCII, empty, wide, combining,
    /// tabbed.
    const TEXTS: [(&str, usize); 9] = [
        ("a", 1),
        ("bc", 2),
        ("", 0),
        (",", 1),
        ("0123456789", 10),
        ("中", 2),
        ("é", 1),
        ("e\u{301}", 1),
        ("a\tb", 4),
    ];

    const SOURCE: &str = "héllo wörld 中文 plain";

    fn generate(a: &DocArena, rng: &mut Rng, depth: u32, flat: bool, aligned: bool) -> Generated {
        let leaf = |doc, cost, accepted| Generated {
            doc,
            cost,
            accepted,
        };
        let pick = if depth == 0 {
            rng.below(40)
        } else {
            rng.below(100)
        };
        match pick {
            0..=13 => {
                let (s, w) = TEXTS[rng.below(4)];
                leaf(a.text(s), w, true)
            }
            14..=19 => {
                let (s, w) = TEXTS[rng.below(TEXTS.len())];
                leaf(a.text_pooled(s), w, true)
            }
            20..=22 => {
                let (span, w) = [
                    (Span::new(0, 6), 5),
                    (Span::new(7, 13), 5),
                    (Span::new(14, 20), 4),
                    (Span::new(21, 26), 5),
                ][rng.below(4)];
                leaf(a.source_span(span, SOURCE), w, true)
            }
            23..=29 => leaf(a.line(), 1, flat),
            30..=33 => leaf(a.softline(), 0, flat),
            34 => leaf(a.hardline(), 0, false),
            35 => leaf(a.literalline(), 0, false),
            36 => leaf(a.text_pooled("n\nl"), 0, false),
            37 => leaf(a.multiline_text("m\nl"), 0, false),
            38 => leaf(a.break_parent(), 0, false),
            39 => leaf(a.line_suffix_boundary(), 0, false),
            40..=69 => {
                let is_fill = pick >= 60;
                let flat = flat || is_fill;
                let count = rng.below(6);
                let mut kids = Vec::new();
                let mut cost = 0;
                let mut accepted = true;
                for _ in 0..count {
                    let kid = generate(a, rng, depth - 1, flat, aligned);
                    kids.push(kid.doc);
                    cost += kid.cost;
                    accepted &= kid.accepted;
                }
                let doc = if is_fill {
                    a.fill(&kids)
                } else {
                    a.concat(&kids)
                };
                leaf(doc, cost, accepted)
            }
            70..=81 => {
                let inner = generate(a, rng, depth - 1, true, aligned);
                leaf(a.group(inner.doc), inner.cost, inner.accepted)
            }
            82..=87 => {
                // The renderer does not support a dedent inside a sub-tab align run
                // (a debug assertion in `RenderIndent`), so none is generated there.
                let kind = rng.below(if aligned { 3 } else { 4 });
                let inner = generate(a, rng, depth - 1, flat, aligned || kind == 1);
                let doc = match kind {
                    0 => a.indent(inner.doc),
                    1 => a.align(1 + rng.below(3) as u32, inner.doc),
                    2 => a.align_root(rng.below(3), inner.doc),
                    _ => a.dedent(inner.doc),
                };
                leaf(doc, inner.cost, inner.accepted)
            }
            88..=93 => {
                let inner = generate(a, rng, depth - 1, flat, aligned);
                let reserve = rng.below(4);
                leaf(
                    a.with_context(inner.doc, DocContext::reserving(reserve)),
                    inner.cost + reserve,
                    inner.accepted,
                )
            }
            94 => {
                let inner = generate(a, rng, depth - 1, true, aligned);
                leaf(a.group_break(inner.doc), 0, false)
            }
            95 => {
                let one = generate(a, rng, depth - 1, true, aligned);
                let two = generate(a, rng, depth - 1, true, aligned);
                leaf(a.conditional_group(&[one.doc, two.doc]), 0, false)
            }
            96..=97 => {
                let one = generate(a, rng, depth - 1, flat, aligned);
                let two = generate(a, rng, depth - 1, flat, aligned);
                leaf(a.if_break(one.doc, two.doc), 0, false)
            }
            98 => {
                let inner = generate(a, rng, depth - 1, true, aligned);
                leaf(a.line_suffix(inner.doc), 0, false)
            }
            _ => {
                let inner = generate(a, rng, depth - 1, true, aligned);
                leaf(a.group_with_id(inner.doc).doc(), 0, false)
            }
        }
    }

    /// Generated docs through both paths, at widths around what each needs.
    ///
    /// Three things are asserted per doc and position. The walk's verdict is the one
    /// the generator states independently of the walk's code (the same rule, written a
    /// second time, so a slip in either shows); the renderer is the oracle for the
    /// bytes. A doc is accepted exactly when it holds only accepted shapes and its
    /// charge fits. An accepted doc's bytes are the renderer's (the twin asserts that
    /// too, and the column). And a doc the renderer breaks a line in is never accepted.
    #[test]
    fn generated_docs_agree_with_the_renderer() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut accepted = 0u32;
        let mut refused_broken = 0u32;
        let mut refused_flat = 0u32;
        let mut a = DocArena::new();
        for round in 0..40_000 {
            if round % 64 == 0 {
                a.reset();
            }
            let depth = 1 + rng.below(5) as u32;
            let generated = generate(&a, &mut rng, depth, false, false);
            let column = rng.below(12);
            let embed = EmbedContext {
                suffix_width: rng.below(4),
                // Past the column, the renderer reserves no suffix; the walk always does.
                first_line_offset: [0, 0, 40][rng.below(3)],
                base_indent_offset: rng.below(2),
                ..EmbedContext::default()
            };
            let need = column + generated.cost + embed.suffix_width;
            for print_width in [
                need.saturating_sub(2),
                need.saturating_sub(1),
                need,
                need + 1,
                need + 30,
            ] {
                let render = width(print_width);
                // Saturating, as the renderer's own budget is: a doc that charges
                // nothing is accepted however far past the width it starts.
                let budget = print_width
                    .saturating_sub(embed.suffix_width)
                    .saturating_sub(column);
                let expect_accept = generated.accepted && generated.cost <= budget;
                let wrote = flat(&a, generated.doc, &render, &embed, column, SOURCE);
                assert_eq!(
                    wrote.is_some(),
                    expect_accept,
                    "round {round}: verdict at width {print_width} (column {column}, \
                     cost {}, accepted shapes {})",
                    generated.cost,
                    generated.accepted
                );
                let reference = rendered(&a, generated.doc, &render, &embed, column, SOURCE);
                match wrote {
                    Some(wrote) => {
                        assert_eq!(
                            wrote, reference,
                            "round {round}: bytes at width {print_width}"
                        );
                        accepted += 1;
                    }
                    None if reference.contains('\n') => refused_broken += 1,
                    None => refused_flat += 1,
                }
            }
        }
        // The generator must reach all three outcomes in bulk, or the test grades
        // nothing.
        assert!(accepted > 20_000, "accepted {accepted}");
        assert!(
            refused_broken > 20_000,
            "refused, renderer broke: {refused_broken}"
        );
        assert!(
            refused_flat > 2000,
            "refused, renderer flat: {refused_flat}"
        );
    }
}
