//! Width fitting algorithms for arena-based doc trees.

use smallvec::SmallVec;

use super::arena::{
    ArenaCommand, DocArena, DocId, DocNode, LAYOUT_UNKNOWN, LAYOUT_WIDTH_MAX, RenderIndent,
};
use super::types::{LineKind, Mode};

/// Flat-mode width of a subtree for the `arena_fits` fast-path, read out of the
/// arena's subtree-layout memo. `Some(w)` = break-free subtree occupying `w`
/// columns flat; `None` = the walk must visit it (it forces a break, or it
/// carries state the walk needs). Mirrors the flat-mode arm of the fits loop
/// exactly, so substituting `remaining -= w` for the walk is byte-identical.
///
/// An inline probe over [`DocArena::subtree_layout_fill`]: the fits walk reads an
/// already-warm slot far more often than it fills one — 98.9% of the fills are
/// already done by the build-time `will_break` walk that shares this memo — so
/// the warm path is a load and **one unsigned compare** at the call site
/// (`LAYOUT_WIDTH_MAX` sits below both break sentinels and the unknown one
/// precisely so that the common case is that single test).
#[inline]
fn flat_width_memo(
    id: DocId,
    nodes: &[DocNode],
    children: &[DocId],
    cache: &mut [u32],
) -> Option<u32> {
    let v = cache[id.index()];
    if v <= LAYOUT_WIDTH_MAX {
        Some(v)
    } else if v == LAYOUT_UNKNOWN {
        layout_to_width(DocArena::subtree_layout_fill(id, nodes, children, cache))
    } else {
        None
    }
}

/// A packed layout cell as the fits walk reads it: a width, or "walk it".
#[inline]
fn layout_to_width(v: u32) -> Option<u32> {
    if v <= LAYOUT_WIDTH_MAX { Some(v) } else { None }
}

/// Check if a doc fits in the remaining width, looking ahead at remaining commands.
///
/// `has_line_suffix` is the caller's pending-suffix state (Prettier's
/// `hasLineSuffix`, passed as `lineSuffix.length > 0`): a deferred comment
/// already queued for this line. Reaching a `LineSuffixBoundary` with one
/// pending doesn't fit — the boundary will end the line to flush it, so a group
/// measured flat across it would render a break it never accounted for. The
/// walk arms an analogous flush-scoped state at a [`DocNode::FlushBreak`] and
/// carries it into the `rest_commands` look-ahead the same way — see
/// `pending_flush` below.
///
/// Takes no `EmbedContext`: a fits decision needs only the fixed
/// [`crate::TAB_WIDTH`], and the embed context's one width effect
/// (`effective_suffix_width`) is already folded into `remaining_width` by the
/// caller.
pub(super) fn arena_fits_with_lookahead(
    arena: &DocArena,
    doc: DocId,
    mode: Mode,
    rest_commands: &[ArenaCommand],
    remaining_width: isize,
    mut has_line_suffix: bool,
) -> bool {
    if remaining_width == isize::MAX {
        return true;
    }

    let nodes = arena.borrow_nodes();
    let children_vec = arena.borrow_children();
    let mut layout_cache = arena.borrow_layout_cache();
    if layout_cache.len() < nodes.len() {
        layout_cache.resize(nodes.len(), LAYOUT_UNKNOWN);
    }
    let mut remaining = remaining_width;
    if remaining < 0 {
        return false;
    }

    let mut stack: SmallVec<[(DocId, Mode); 16]> = SmallVec::new();
    let mut rest_idx = rest_commands.len();

    // Pending flush-scoped break ([`DocNode::FlushBreak`]): a deferred trailing
    // run behind this point needs a line end, so a *flat* breakable line — a
    // `Line(Normal|Soft)`, or an `IfBreak` whose break arm can break — reached
    // while pending does not fit: the group owning that line must break to
    // flush the run. A group with no line opportunity after the node is
    // unaffected and stays flat (the whole point — an unscoped `BreakParent`
    // here forced intermediate groups into breaks the reparse cannot
    // reproduce). Discovered by walking, never seeded: the group whose verdict
    // must flip always contains the node (the memo returns `None` on any
    // subtree holding one, so the walk always sees it).
    let mut pending_flush = false;

    // Tail-continuation dispatch — same shape as the render loops (see
    // `render_doc_iterative`): single-continuation arms assign the current
    // `(id, mode)` and `continue` instead of a push+pop round trip through the
    // stack; width-consuming terminal arms fall through to the `remaining`
    // check + pop at the bottom, preserving the original between-items check
    // placement. `WithContext` both consumes width AND forwards, so it keeps
    // an inline `remaining < 0` check before its continuation (a hardline in
    // the child must not flip a false verdict to true).
    let (mut current_id, mut current_mode) = (doc, mode);

    loop {
        // Fast path: a break-free subtree in flat mode contributes a fixed,
        // memoized width — identical to walking it (the walk would only sum the
        // same width with no early return). Bypassed while a flush-scoped break
        // is pending: the memo summarizes a `Line(Normal)` as width 1, hiding
        // exactly the node the pending state must veto on.
        if current_mode == Mode::Flat
            && !pending_flush
            && let Some(w) = flat_width_memo(
                current_id,
                &nodes,
                &children_vec,
                layout_cache.as_mut_slice(),
            )
        {
            remaining -= w as isize;
        } else {
            match &nodes[current_id.index()] {
                // Reached only in Break mode (the Flat-mode fast path above already
                // consulted the memo). A text's flat width is mode-independent, so
                // the memo applies here too — going through the cache keeps the
                // node dispatch off the repeat Break-mode visits.
                DocNode::Text(_) => match flat_width_memo(
                    current_id,
                    &nodes,
                    &children_vec,
                    layout_cache.as_mut_slice(),
                ) {
                    Some(w) => remaining -= w as isize,
                    // Newline-bearing text ends the line — everything so far fit.
                    None => return true,
                },

                DocNode::MultilineText { first_width, .. } => {
                    // Equivalent to walking `[Text(first_line), Line(Hard), …]`: the
                    // first line's width counts, then the first newline ends the line
                    // (a hardline returns true in either mode). `remaining >= 0`
                    // distinguishes the two loop outcomes: ≥0 → the next item would be
                    // the hardline → true; <0 → the bottom check would return false.
                    // The width is precomputed at build (clamped — verdict-preserving,
                    // print width is orders of magnitude below the clamp), so no pool
                    // read happens here.
                    remaining -= *first_width as isize;
                    return remaining >= 0;
                }

                // Any `Line` reaching the slow walk ends the current line, so
                // everything measured so far fits. `Hard`/`Literal` break in
                // either mode; a `Soft`/`Normal` reaches here only in `Break`
                // mode — where the break likewise ends the line — or in Flat
                // mode with a flush-scoped break pending (the memo fast path
                // answers them `Some(0)`/`Some(1)` otherwise). A flat
                // `Soft`/`Normal` renders no line end, so while pending it is
                // the veto point: the group must break here to flush the run.
                DocNode::Line(kind) => {
                    return !(pending_flush
                        && current_mode == Mode::Flat
                        && matches!(kind, LineKind::Soft | LineKind::Normal));
                }

                DocNode::Group {
                    contents,
                    expanded_states,
                    should_break,
                    ..
                } => {
                    let mode_for_group = if *should_break {
                        Mode::Break
                    } else {
                        current_mode
                    };
                    let doc_to_check = if mode_for_group == Mode::Break {
                        if !expanded_states.is_empty() {
                            let kids = expanded_states.resolve(&children_vec);
                            *kids.last().unwrap_or(contents)
                        } else {
                            *contents
                        }
                    } else {
                        *contents
                    };
                    (current_id, current_mode) = (doc_to_check, mode_for_group);
                    continue;
                }

                DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                    current_id = *inner;
                    continue;
                }

                DocNode::AlignRoot { contents, .. } | DocNode::Align { contents, .. } => {
                    current_id = *contents;
                    continue;
                }

                DocNode::IndentIfBreak { contents, .. } => {
                    current_id = *contents;
                    continue;
                }

                DocNode::IfBreak {
                    break_doc,
                    flat_doc,
                    group_id,
                } => {
                    // A group-id if_break keys on a group that, during this
                    // hypothetical fits test, is still unresolved → treat as flat.
                    // This keeps trailing text (e.g. a block head's `}`) counted in
                    // the keyed group's own width so it breaks at the right boundary.
                    let chosen = if group_id.is_none() && current_mode == Mode::Break {
                        *break_doc
                    } else {
                        // A flat if_break renders no line end, but its break arm may
                        // hold one (a composite's `if_break(line + "| ", " | ")`
                        // separator): with a flush-scoped break pending, that unmade
                        // line is where the deferred run flushes, so the group
                        // measured flat across it does not fit — it must break to
                        // take the break arm. Scoped to plain if_breaks: a group-id
                        // one keys on another group's decision, which breaking the
                        // measured group would not change. Mode here is necessarily
                        // Flat for a plain if_break — the Break case chose
                        // `break_doc` above — so no explicit mode check is needed.
                        if pending_flush
                            && group_id.is_none()
                            && DocArena::can_break_inner(*break_doc, &nodes, &children_vec)
                        {
                            return false;
                        }
                        *flat_doc
                    };
                    current_id = chosen;
                    continue;
                }

                DocNode::Concat(range) | DocNode::Fill(range) => {
                    let kids = range.resolve(&children_vec);
                    if let Some((&first, rest)) = kids.split_first() {
                        for &child in rest.iter().rev() {
                            stack.push((child, current_mode));
                        }
                        current_id = first;
                        continue;
                    }
                }

                DocNode::WithContext { doc, context } => {
                    remaining -= context.trailing_reserve() as isize;
                    if remaining < 0 {
                        return false;
                    }
                    current_id = *doc;
                    continue;
                }

                // Zero columns each — a suffix is deferred to the line's end, and
                // the boundary is the flush point. What they carry is state: a
                // boundary reached with a suffix pending will END THE LINE to
                // flush it, so nothing measured past it is on this line.
                DocNode::LineSuffix(_) => has_line_suffix = true,
                DocNode::LineSuffixBoundary => {
                    if has_line_suffix {
                        return false;
                    }
                }
                DocNode::BreakParent => return false,
                // Zero columns; arms the pending-flush veto above. Not an
                // unconditional "doesn't fit" — a group with no line
                // opportunity after this point is deliberately left flat.
                DocNode::FlushBreak => pending_flush = true,
                // Render-only sentinel: zero columns, no layout effect — the probe it
                // completes exists only on the real render's command stack.
                DocNode::FlowProbeEnd => {}

                // Transparent to contents: only conditional-group state
                // selection reads the probe (arena_render's states loop); a
                // fits walk that reaches the node measures what would render.
                DocNode::GatedState { contents, .. } => {
                    current_id = *contents;
                    continue;
                }
            }
        }

        // Terminal arm: check the accumulated width, then take the next item —
        // from the stack, else from the look-ahead rest commands (back to
        // front), else everything fit.
        if remaining < 0 {
            return false;
        }
        (current_id, current_mode) = match stack.pop() {
            Some(next) => next,
            None => {
                if rest_idx == 0 {
                    return true;
                }
                rest_idx -= 1;
                let cmd = &rest_commands[rest_idx];
                (cmd.doc, cmd.mode)
            }
        };
    }
}

/// Check if a doc fits in the remaining width (public API without look-ahead).
///
/// Uses the production [`crate::TAB_WIDTH`] for visual width calculations.
/// Internal callers that need look-ahead use [`arena_fits_with_lookahead`]
/// directly. Build-time callers have no render loop and so no pending line
/// suffix — hence `has_line_suffix: false`.
pub fn arena_fits(arena: &DocArena, doc: DocId, width: usize, mode: Mode) -> bool {
    arena_fits_with_lookahead(arena, doc, mode, &[], width as isize, false)
}

/// Check if multiple docs fit sequentially in the remaining width.
///
/// Thin wrapper over [`arena_fits_with_lookahead`]: the first doc is the main
/// doc, the rest ride as look-ahead rest commands (consumed back-to-front by
/// the walk, hence the reversed collect; their `indent` is unread there).
/// Replaces what was a full copy of the fits walk that had drifted — it
/// lacked the `flat_width_memo` fast path and its `Group` arm ignored
/// `should_break`/`expanded_states`.
pub(super) fn arena_fits_multi(
    arena: &DocArena,
    doc_ids: &[DocId],
    width: usize,
    mode: Mode,
    has_line_suffix: bool,
) -> bool {
    if width == usize::MAX {
        return true;
    }
    let Some((&first, rest)) = doc_ids.split_first() else {
        return true;
    };
    let rest_commands: SmallVec<[ArenaCommand; 4]> = rest
        .iter()
        .rev()
        .map(|&doc| ArenaCommand {
            indent: RenderIndent::default(),
            mode,
            doc,
        })
        .collect();
    arena_fits_with_lookahead(
        arena,
        first,
        mode,
        &rest_commands,
        width as isize,
        has_line_suffix,
    )
}

#[cfg(test)]
mod break_mode_fits_tests {
    //! Boundary contract for the `arena_fits_with_lookahead` **Break-mode slow
    //! walk**. The `fits_flat` / `assert_flat_width` guards in `doc::mod.rs` cover
    //! only Flat mode, where the `flat_width_memo` fast path answers before the
    //! walk runs — so the Break-mode width-accounting arms (a `Text`, a
    //! `MultilineText` first line, an `IfBreak`, a `WithContext` trailing reserve)
    //! had no assertion, and `cargo mutants` flagged their arithmetic as surviving.
    //!
    //! No corpus grades this: a fits verdict changes the *output* only when it
    //! lands exactly on the print-width boundary, so an off-by-one in a
    //! width-subtraction arm is invisible to the fixtures and any format/wire diff
    //! ([`super::super::arena`]'s `pooled_text_width_tests` documents the same
    //! blind spot). Each case pins the exact fit/no-fit boundary; break an arm and
    //! watch one assertion flip.
    use super::super::DocContext;
    use super::super::arena::{DocArena, DocId};
    use super::super::types::Mode;
    use super::arena_fits;

    /// Fit `doc` in `width` columns in Break mode.
    fn fits_break(a: &DocArena, doc: DocId, width: usize) -> bool {
        arena_fits(a, doc, width, Mode::Break)
    }

    /// The doc fits at `w` but not at `w - 1`: any off-by-one in a width arm flips
    /// exactly one of these.
    fn assert_break_boundary(a: &DocArena, doc: DocId, w: usize) {
        assert!(
            fits_break(a, doc, w),
            "expected width {w} to fit in break mode"
        );
        assert!(
            !fits_break(a, doc, w - 1),
            "expected width {} not to fit in break mode",
            w - 1
        );
    }

    #[test]
    fn break_mode_text_consumes_its_width() {
        // Break-mode `Text` arm (`remaining -= w`): a 4-col text fits in 4, not 3.
        let a = DocArena::new();
        assert_break_boundary(&a, a.text("abcd"), 4);
        // Tab expansion is part of the width (TAB_WIDTH = 2 → "a\tb" is 4 cols).
        let a2 = DocArena::new();
        assert_break_boundary(&a2, a2.text_pooled("a\tb"), 4);
    }

    #[test]
    fn break_mode_multiline_text_measures_first_line() {
        // `MultilineText` arm (`remaining -= first_width; return remaining >= 0`):
        // only the first line ("abcd", 4 cols) counts — the newline ends the line,
        // so the tail's width is irrelevant. The `>= 0` verdict is exact at the
        // boundary (remaining 0 must still fit).
        let a = DocArena::new();
        let ml = a.multiline_text("abcd\na much longer trailing line that is ignored");
        assert!(
            fits_break(&a, ml, 4),
            "first line fits exactly (remaining 0)"
        );
        assert!(
            !fits_break(&a, ml, 3),
            "first line overflows (remaining -1)"
        );
    }

    #[test]
    fn break_mode_if_break_measures_break_doc() {
        // `IfBreak` with no group id in Break mode measures `break_doc` (4 cols),
        // never `flat_doc` (1 col) — the `group_id.is_none() && mode == Break`
        // selector. A mutated selector would measure the 1-col flat form and
        // wrongly fit at 3.
        let a = DocArena::new();
        let doc = a.if_break(a.text("WWWW"), a.text("y"));
        assert_break_boundary(&a, doc, 4);
    }

    #[test]
    fn break_mode_with_context_reserves_trailing_width() {
        // `WithContext` arm reserves `trailing_reserve` up front
        // (`remaining -= reserve`, then an inline `remaining < 0` guard) before
        // descending: 4 content + 3 reserved = 7.
        let a = DocArena::new();
        let doc = a.with_context(a.text("abcd"), DocContext::reserving(3));
        assert_break_boundary(&a, doc, 7);
    }
}
