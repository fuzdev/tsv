//! Width fitting algorithms for arena-based doc trees.

use crate::EmbedContext;
use crate::config::TAB_WIDTH;
use crate::printing::visual_width;
use smallvec::SmallVec;

use super::arena::{ArenaCommand, DocArena, DocId, DocNode, FLAT_WIDTH_BREAKS, FLAT_WIDTH_UNKNOWN};
use super::types::{CachedWidth, DocText, LineKind, Mode, TextResolver, resolve_text};

/// Flat width of a text node, or `None` when the text contains a newline (the
/// line ends inside it, so it has no single-line width). The one definition of
/// the cached-or-measure fallback, backing [`flat_width_fill`]'s `Text` arm —
/// its only caller, since the fits walk's `Text` arm reaches it via the memo.
#[inline]
fn text_flat_width<R: TextResolver + ?Sized>(t: &DocText, resolver: Option<&R>) -> Option<u32> {
    match t.cached_width() {
        CachedWidth::Width(w) => Some(u32::from(w)),
        CachedWidth::HasNewline => None,
        CachedWidth::NotComputed => {
            let s = resolve_text(t, resolver);
            if s.contains('\n') {
                None
            } else {
                Some(visual_width(s, TAB_WIDTH) as u32)
            }
        }
    }
}

/// Flat-mode width of a subtree for the `arena_fits` fast-path, memoized per
/// `DocId`. `Some(w)` = break-free subtree occupying `w` columns flat; `None` =
/// contains a forced break, so `arena_fits` must walk it. Mirrors the flat-mode
/// arm of the fits loop exactly, so substituting `remaining -= w` for the walk
/// is byte-identical.
///
/// Split into an inline cache probe over an outlined recursive fill: the fits
/// walk probes an already-warm slot far more often than it fills one, so the
/// warm path is a load + compare at the call site instead of a full call.
#[inline]
fn flat_width_memo<R: TextResolver + ?Sized>(
    id: DocId,
    nodes: &[DocNode],
    children: &[DocId],
    cache: &mut [u32],
    resolver: Option<&R>,
) -> Option<u32> {
    match cache[id.index()] {
        FLAT_WIDTH_UNKNOWN => flat_width_fill(id, nodes, children, cache, resolver),
        FLAT_WIDTH_BREAKS => None,
        w => Some(w),
    }
}

/// The cold half of [`flat_width_memo`]: compute and cache a subtree's flat
/// width. Runs at most once per node; recursion goes back through the inline
/// probe so warm children never re-enter here.
#[cold]
#[inline(never)]
fn flat_width_fill<R: TextResolver + ?Sized>(
    id: DocId,
    nodes: &[DocNode],
    children: &[DocId],
    cache: &mut [u32],
    resolver: Option<&R>,
) -> Option<u32> {
    let result: Option<u32> = match &nodes[id.index()] {
        DocNode::Text(t) => text_flat_width(t, resolver),
        // Contains hardlines → no break-free flat width (like a newline-bearing
        // `Text` or a `Line(Hard)`); force the `arena_fits` walk.
        DocNode::MultilineText(_) => None,
        DocNode::Line(kind) => match kind {
            LineKind::Hard | LineKind::Literal => None,
            LineKind::Soft => Some(0),
            LineKind::Normal => Some(1),
        },
        DocNode::Group {
            contents,
            should_break,
            ..
        } => {
            if *should_break {
                None
            } else {
                flat_width_memo(*contents, nodes, children, cache, resolver)
            }
        }
        DocNode::IsolatedGroup { contents } => {
            flat_width_memo(*contents, nodes, children, cache, resolver)
        }
        DocNode::Indent(inner) | DocNode::Dedent(inner) => {
            flat_width_memo(*inner, nodes, children, cache, resolver)
        }
        DocNode::Align { contents, .. } => {
            flat_width_memo(*contents, nodes, children, cache, resolver)
        }
        DocNode::IndentIfBreak { contents, .. } => {
            flat_width_memo(*contents, nodes, children, cache, resolver)
        }
        DocNode::IfBreak { flat_doc, .. } => {
            flat_width_memo(*flat_doc, nodes, children, cache, resolver)
        }
        DocNode::Concat(range) | DocNode::Fill(range) => {
            let kids = range.resolve(children);
            let mut sum: u32 = 0;
            let mut ok = true;
            for &kid in kids {
                match flat_width_memo(kid, nodes, children, cache, resolver) {
                    Some(w) => sum = sum.saturating_add(w),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok { Some(sum) } else { None }
        }
        DocNode::WithContext { doc, context } => {
            flat_width_memo(*doc, nodes, children, cache, resolver)
                .map(|w| w.saturating_add(context.trailing_reserve as u32))
        }
        DocNode::LineSuffix(_) | DocNode::LineSuffixBoundary => Some(0),
        DocNode::BreakParent => None,
    };
    cache[id.index()] = match result {
        Some(w) => w,
        None => FLAT_WIDTH_BREAKS,
    };
    result
}

/// Check if a doc fits in the remaining width, looking ahead at remaining commands.
///
/// `embed` is currently unused — fits decisions only need the fixed
/// [`crate::TAB_WIDTH`]. The parameter is threaded so internal callers from
/// `arena_render` can pass it uniformly.
pub(super) fn arena_fits_with_lookahead<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    mode: Mode,
    rest_commands: &[ArenaCommand],
    remaining_width: isize,
    _embed: &EmbedContext,
    resolver: Option<&R>,
) -> bool {
    if remaining_width == isize::MAX {
        return true;
    }

    let nodes = arena.borrow_nodes();
    let children_vec = arena.borrow_children();
    let mut flat_cache = arena.borrow_flat_width_cache();
    if flat_cache.len() < nodes.len() {
        flat_cache.resize(nodes.len(), FLAT_WIDTH_UNKNOWN);
    }
    let mut remaining = remaining_width;

    let mut stack: SmallVec<[(DocId, Mode); 16]> = SmallVec::new();
    stack.push((doc, mode));

    let mut rest_idx = rest_commands.len();

    while remaining >= 0 {
        let Some((current_id, current_mode)) = stack.pop() else {
            if rest_idx == 0 {
                return true;
            }
            rest_idx -= 1;
            let cmd = &rest_commands[rest_idx];
            stack.push((cmd.doc, cmd.mode));
            continue;
        };

        // Fast path: a break-free subtree in flat mode contributes a fixed,
        // memoized width — identical to walking it (the walk would only sum the
        // same width with no early return).
        if current_mode == Mode::Flat
            && let Some(w) = flat_width_memo(
                current_id,
                &nodes,
                &children_vec,
                flat_cache.as_mut_slice(),
                resolver,
            )
        {
            remaining -= w as isize;
            continue;
        }

        match &nodes[current_id.index()] {
            // Reached only in Break mode (the Flat-mode fast path above already
            // consulted the memo). A text's flat width is mode-independent, so
            // the memo applies here too — caching the resolve+measure that
            // Break-mode visits would otherwise repeat per fits call.
            DocNode::Text(_) => match flat_width_memo(
                current_id,
                &nodes,
                &children_vec,
                flat_cache.as_mut_slice(),
                resolver,
            ) {
                Some(w) => remaining -= w as isize,
                // Newline-bearing text ends the line — everything so far fit.
                None => return true,
            },

            DocNode::MultilineText(s) => {
                // Equivalent to walking `[Text(first_line), Line(Hard), …]`: the
                // first line's width counts, then the first newline ends the line
                // (a hardline returns true in either mode). `remaining >= 0`
                // distinguishes the two loop outcomes: ≥0 → the next pop would be
                // the hardline → true; <0 → `while remaining >= 0` exits → false.
                let first = s.split('\n').next().unwrap_or("");
                remaining -= visual_width(first, TAB_WIDTH) as isize;
                return remaining >= 0;
            }

            DocNode::Line(kind) => match kind {
                LineKind::Hard | LineKind::Literal => return true,
                _ if current_mode == Mode::Break => return true,
                LineKind::Soft => {}
                LineKind::Normal => {
                    remaining -= 1;
                }
            },

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
                stack.push((doc_to_check, mode_for_group));
            }

            DocNode::IsolatedGroup { contents } => {
                stack.push((*contents, current_mode));
            }

            DocNode::Indent(inner) | DocNode::Dedent(inner) => {
                stack.push((*inner, current_mode));
            }

            DocNode::Align { contents, .. } => {
                stack.push((*contents, current_mode));
            }

            DocNode::IndentIfBreak { contents, .. } => {
                stack.push((*contents, current_mode));
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
                    *flat_doc
                };
                stack.push((chosen, current_mode));
            }

            DocNode::Concat(range) => {
                let kids = range.resolve(&children_vec);
                for &child in kids.iter().rev() {
                    stack.push((child, current_mode));
                }
            }

            DocNode::Fill(range) => {
                let kids = range.resolve(&children_vec);
                for &child in kids.iter().rev() {
                    stack.push((child, current_mode));
                }
            }

            DocNode::WithContext { doc, context } => {
                remaining -= context.trailing_reserve as isize;
                stack.push((*doc, current_mode));
            }

            DocNode::LineSuffix(_) => {}
            DocNode::LineSuffixBoundary => {}
            DocNode::BreakParent => return false,
        }
    }

    false
}

/// Check if a doc fits in the remaining width (public API without look-ahead).
///
/// Uses the production [`crate::TAB_WIDTH`] for visual width calculations.
/// Internal callers that need look-ahead use [`arena_fits_with_lookahead`]
/// directly.
pub fn arena_fits<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    width: usize,
    mode: Mode,
    resolver: Option<&R>,
) -> bool {
    arena_fits_with_lookahead(
        arena,
        doc,
        mode,
        &[],
        width as isize,
        &EmbedContext::default(),
        resolver,
    )
}

/// Check if multiple docs fit sequentially in the remaining width.
///
/// Thin wrapper over [`arena_fits_with_lookahead`]: the first doc is the main
/// doc, the rest ride as look-ahead rest commands (consumed back-to-front by
/// the walk, hence the reversed collect; their `indent` is unread there).
/// Replaces what was a full copy of the fits walk that had drifted — it
/// lacked the `flat_width_memo` fast path and its `Group` arm ignored
/// `should_break`/`expanded_states`.
pub(super) fn arena_fits_multi<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc_ids: &[DocId],
    width: usize,
    mode: Mode,
    embed: &EmbedContext,
    resolver: Option<&R>,
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
            indent: 0,
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
        embed,
        resolver,
    )
}
