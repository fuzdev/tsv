//! Width fitting algorithms for arena-based doc trees.

use crate::EmbedContext;
use crate::printing::visual_width;
use smallvec::SmallVec;

use super::arena::{ArenaCommand, DocArena, DocId, DocNode};
use super::render_config::RenderConfig;
use super::types::{LineKind, Mode, TEXT_WIDTH_HAS_NEWLINE, TextResolver, resolve_text};

/// Check if a doc fits in the remaining width, looking ahead at remaining commands.
///
/// Arena-based version of `fits_with_lookahead`.
///
/// `embed` is currently unused — fits decisions only need `tab_width`. The
/// parameter is threaded so internal callers from `arena_render` can pass
/// the same render/embed pair uniformly.
#[allow(clippy::too_many_arguments)]
pub(super) fn arena_fits_with_lookahead<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    mode: Mode,
    rest_commands: &[ArenaCommand],
    remaining_width: isize,
    render: &RenderConfig,
    _embed: &EmbedContext,
    resolver: Option<&R>,
) -> bool {
    if remaining_width == isize::MAX {
        return true;
    }

    let nodes = arena.borrow_nodes();
    let children_vec = arena.borrow_children();
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
                        remaining -= visual_width(s, render.tab_width) as isize;
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
            } => {
                let chosen = if current_mode == Mode::Break {
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
/// Internal callers that need to vary widths should use
/// [`arena_fits_with_lookahead`] with a custom [`RenderConfig`].
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
        &RenderConfig::default(),
        &EmbedContext::default(),
        resolver,
    )
}

/// Check if multiple docs fit sequentially in the remaining width.
#[allow(clippy::too_many_arguments)]
pub(super) fn arena_fits_multi<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc_ids: &[DocId],
    width: usize,
    mode: Mode,
    render: &RenderConfig,
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
                        remaining_width -= visual_width(s, render.tab_width) as isize;
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
            } => {
                let chosen = if current_mode == Mode::Break {
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
#[inline]
pub(super) fn update_pos_for_text(pos: &mut usize, s: &str, tab_width: usize) {
    if let Some(last_newline_pos) = s.rfind('\n') {
        let after_newline = &s[last_newline_pos + 1..];
        *pos = visual_width(after_newline, tab_width);
    } else {
        *pos += visual_width(s, tab_width);
    }
}
