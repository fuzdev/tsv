//! Width fitting algorithms for arena-based doc trees.

use crate::EmbedContext;
use crate::config::TAB_WIDTH;
use crate::printing::visual_width;
use smallvec::SmallVec;

use super::arena::{ArenaCommand, DocArena, DocId, DocNode, FLAT_WIDTH_BREAKS, FLAT_WIDTH_UNKNOWN};
use super::types::{LineKind, Mode, TEXT_WIDTH_HAS_NEWLINE, TextResolver, resolve_text};

/// Flat-mode width of a subtree for the `arena_fits` fast-path, memoized per
/// `DocId`. `Some(w)` = break-free subtree occupying `w` columns flat; `None` =
/// contains a forced break, so `arena_fits` must walk it. Mirrors the flat-mode
/// arm of the fits loop exactly, so substituting `remaining -= w` for the walk
/// is byte-identical.
fn flat_width_memo<R: TextResolver + ?Sized>(
    id: DocId,
    nodes: &[DocNode],
    children: &[DocId],
    cache: &mut [u32],
    resolver: Option<&R>,
) -> Option<u32> {
    match cache[id.index()] {
        FLAT_WIDTH_UNKNOWN => {}
        FLAT_WIDTH_BREAKS => return None,
        w => return Some(w),
    }
    let result: Option<u32> = match &nodes[id.index()] {
        DocNode::Text(t) => match t.cached_width() {
            Some(w) if w == TEXT_WIDTH_HAS_NEWLINE => None,
            Some(w) => Some(u32::from(w)),
            None => {
                let s = resolve_text(t, resolver);
                if s.contains('\n') {
                    None
                } else {
                    Some(visual_width(s, TAB_WIDTH) as u32)
                }
            }
        },
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
/// Arena-based version of `fits_with_lookahead`.
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
            DocNode::Text(t) => {
                match t.cached_width() {
                    Some(w) if w == TEXT_WIDTH_HAS_NEWLINE => return true,
                    Some(w) => remaining -= w as isize,
                    None => {
                        // Not cached (Symbol or ASCII) — fall back to resolve + visual_width
                        let s = resolve_text(t, resolver);
                        if s.contains('\n') {
                            return true;
                        }
                        remaining -= visual_width(s, TAB_WIDTH) as isize;
                    }
                }
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
pub(super) fn arena_fits_multi<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc_ids: &[DocId],
    width: usize,
    mode: Mode,
    _embed: &EmbedContext,
    resolver: Option<&R>,
) -> bool {
    if width == usize::MAX {
        return true;
    }

    let nodes = arena.borrow_nodes();
    let children_vec = arena.borrow_children();
    let mut stack: SmallVec<[(DocId, Mode); 16]> = SmallVec::new();
    let mut remaining_width = width as isize;

    for &doc_id in doc_ids.iter().rev() {
        stack.push((doc_id, mode));
    }

    while let Some((current_id, current_mode)) = stack.pop() {
        match &nodes[current_id.index()] {
            DocNode::Text(t) => {
                match t.cached_width() {
                    Some(w) if w == TEXT_WIDTH_HAS_NEWLINE => return true,
                    Some(w) => {
                        remaining_width -= w as isize;
                        if remaining_width < 0 {
                            return false;
                        }
                    }
                    None => {
                        // Not cached (Symbol or ASCII) — fall back to resolve + visual_width
                        let s = resolve_text(t, resolver);
                        if s.contains('\n') {
                            return true;
                        }
                        remaining_width -= visual_width(s, TAB_WIDTH) as isize;
                        if remaining_width < 0 {
                            return false;
                        }
                    }
                }
            }

            DocNode::Line(kind) => match kind {
                LineKind::Hard | LineKind::Literal => return true,
                _ if current_mode == Mode::Break => return true,
                LineKind::Soft => {}
                LineKind::Normal => {
                    remaining_width -= 1;
                    if remaining_width < 0 {
                        return false;
                    }
                }
            },

            DocNode::Group { contents, .. } => {
                stack.push((*contents, current_mode));
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
                remaining_width -= context.trailing_reserve as isize;
                if remaining_width < 0 {
                    return false;
                }
                stack.push((*doc, current_mode));
            }

            DocNode::LineSuffix(_) => {}
            DocNode::LineSuffixBoundary => {}
            DocNode::BreakParent => return false,
        }
    }

    remaining_width >= 0
}

/// Update position after rendering a text string, accounting for tab expansion.
///
/// The overwhelmingly common input here is short ASCII with no newline — every
/// interned identifier (`Symbol`) and every static punctuation/keyword token
/// reaches this via `render_text`'s uncached-width arm. For those the previous
/// shape scanned the bytes three times (`rfind('\n')` + `visual_width`'s own
/// `is_ascii` + tab count). The fast path below folds the newline reset, tab
/// expansion, and width accumulation into a single forward byte pass, so no
/// backward `memchr` scan runs. The first non-ASCII byte hands off to
/// `update_pos_for_text_unicode` (cold-outlined to keep this fast path lean and
/// inlinable, mirroring `skip_trivia` / `skip_trivia_scan`). Byte-identical to
/// the prior implementation by construction.
#[inline]
pub(super) fn update_pos_for_text(pos: &mut usize, s: &str) {
    let mut col = *pos;
    for &b in s.as_bytes() {
        match b {
            b'\n' => col = 0,
            b'\t' => col += TAB_WIDTH,
            0..=0x7f => col += 1,
            _ => return update_pos_for_text_unicode(pos, s),
        }
    }
    *pos = col;
}

/// Grapheme-aware fallback for `update_pos_for_text` when the text contains a
/// non-ASCII byte. Re-measures the whole string from scratch (the fast path's
/// partial `col` is intentionally dropped) so a combining mark attaching to an
/// ASCII base char is never split mid-grapheme — the original pre-fusion logic,
/// verbatim. Cold: non-ASCII text is rare in the render stream.
#[cold]
#[inline(never)]
fn update_pos_for_text_unicode(pos: &mut usize, s: &str) {
    if let Some(last_newline_pos) = s.rfind('\n') {
        *pos = visual_width(&s[last_newline_pos + 1..], TAB_WIDTH);
    } else {
        *pos += visual_width(s, TAB_WIDTH);
    }
}
