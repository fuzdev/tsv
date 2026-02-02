//! Width fitting algorithms for the doc builder

use crate::PrintConfig;
use crate::printing::visual_width;
use smallvec::SmallVec;

use super::types::{Command, Doc, LineKind, Mode, TextResolver, resolve_text};

/// Check if a doc fits in the remaining width, looking ahead at remaining commands.
///
/// This simulates rendering the doc without actually building the string,
/// tracking the remaining width character-by-character. Returns `true` if
/// the doc fits, `false` otherwise.
///
/// The key innovation from prettier: when the current doc is exhausted,
/// continue checking from `rest_commands`. This enables look-ahead:
/// a group can see what comes after it and make correct breaking decisions.
///
/// Based on prettier's `fits()` algorithm (prettier/src/document/printer/printer.js:52-168).
pub(super) fn fits_with_lookahead<'a, R: TextResolver + ?Sized>(
    doc: &'a Doc,
    mode: Mode,
    rest_commands: &[Command<'a>],
    remaining_width: isize,
    config: &PrintConfig,
    resolver: Option<&R>,
) -> bool {
    if remaining_width == isize::MAX {
        return true; // Infinite width always fits
    }

    let mut remaining = remaining_width;

    // Local stack for processing the doc itself
    let mut stack: SmallVec<[(&Doc, Mode); 16]> = SmallVec::new();
    stack.push((doc, mode));

    // Index into rest_commands (we traverse from end to start, like prettier)
    let mut rest_idx = rest_commands.len();

    while remaining >= 0 {
        // Pop from local stack, or pull from rest_commands if empty
        let Some((current_doc, current_mode)) = stack.pop() else {
            if rest_idx == 0 {
                return true; // Everything checked, it fits
            }
            rest_idx -= 1;
            let cmd = &rest_commands[rest_idx];
            stack.push((cmd.doc, cmd.mode));
            continue;
        };

        match current_doc {
            Doc::Text(t) => {
                let s = resolve_text(t, resolver);
                // If text contains a newline, content after newline is on a new line
                // and always "fits" for this check
                if s.contains('\n') {
                    return true;
                }
                remaining -= visual_width(s, config.tab_width) as isize;
                // Don't return early on negative - let the while condition catch it
            }

            Doc::Line(kind) => {
                match kind {
                    LineKind::Hard | LineKind::Literal => {
                        // Hard/literal lines always break - rest fits on next line
                        return true;
                    }
                    _ if current_mode == Mode::Break => {
                        // Any line in break mode means we're done checking this line
                        return true;
                    }
                    LineKind::Soft => {
                        // Soft line disappears in flat mode
                    }
                    LineKind::Normal => {
                        // Normal line becomes space in flat mode
                        remaining -= 1;
                    }
                }
            }

            Doc::Group {
                contents,
                expanded_states,
                should_break,
                ..
            } => {
                // Prettier's fits() behavior (printer.js lines 74-81):
                // - In Break mode with expandedStates: use last state
                // - In all other cases: use contents (state[0])
                //
                // State selection (trying state[1], state[2], etc.) happens during
                // RENDERING (render.rs lines 265-294), NOT during fits() check.
                //
                // IMPORTANT: When should_break is true (source prefers expanded), the group
                // should be evaluated in Break mode even when the outer context is Flat.
                // This enables the "hug" pattern: `fn('a', 'b', {\n  props\n})` where the
                // head args stay inline but the object breaks. Without this, fits() would
                // check the full inline content and fail, causing all args to expand.
                let mode_for_group = if *should_break {
                    Mode::Break
                } else {
                    current_mode
                };
                let doc_to_check = if mode_for_group == Mode::Break {
                    if let Some(states) = expanded_states {
                        states.last().unwrap_or_else(|| contents.as_ref())
                    } else {
                        contents.as_ref()
                    }
                } else {
                    // Flat mode: always use contents (state[0])
                    contents.as_ref()
                };
                stack.push((doc_to_check, mode_for_group));
            }

            Doc::IsolatedGroup { contents } => {
                // IsolatedGroup behaves like a regular group for fits() checking
                stack.push((contents.as_ref(), current_mode));
            }

            Doc::Indent(inner) | Doc::Dedent(inner) => {
                // Indent/dedent don't affect width in fits() check
                // (indentation only matters at line breaks, which end fits() early)
                stack.push((inner, current_mode));
            }

            Doc::Align { contents, .. } | Doc::AlignSpaces { contents, .. } => {
                // Align/AlignSpaces don't affect width in fits() check
                // (like Indent, indentation only matters at line breaks)
                stack.push((contents, current_mode));
            }

            Doc::IndentIfBreak { contents, .. } => {
                // IndentIfBreak doesn't affect width in fits() check
                // (like Indent, indentation only matters at line breaks)
                // Matches Prettier printer.js line 109: case DOC_TYPE_INDENT_IF_BREAK
                stack.push((contents, current_mode));
            }

            Doc::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if current_mode == Mode::Break {
                    break_doc
                } else {
                    flat_doc
                };
                stack.push((chosen, current_mode));
            }

            Doc::Concat(docs) => {
                // Process docs in reverse order (stack is LIFO)
                for doc in docs.iter().rev() {
                    stack.push((doc, current_mode));
                }
            }

            Doc::Fill(parts) => {
                // For fits() check, Fill behaves like Concat - just check all parts fit
                for doc in parts.iter().rev() {
                    stack.push((doc, current_mode));
                }
            }

            Doc::WithContext { doc, context } => {
                // Extract context and apply trailing_reserve for any doc type.
                // This ensures both Fill and non-Fill docs (like Concat in conditional_group)
                // respect the trailing reserve constraint.
                remaining -= context.trailing_reserve as isize;
                stack.push((doc.as_ref(), current_mode));
            }

            Doc::LineSuffix(_) => {
                // LineSuffix content is NOT counted toward width.
                // This allows trailing comments to be excluded from break decisions.
            }

            Doc::LineSuffixBoundary => {
                // LineSuffixBoundary has no width effect.
            }

            Doc::BreakParent => {
                // BreakParent forces the enclosing group to break
                // In fits check, this means it doesn't fit in flat mode
                return false;
            }
        }
    }

    false // remaining < 0, doesn't fit
}

/// Check if a doc fits in the remaining width (legacy API without look-ahead)
///
/// Note: For most formatting scenarios, the command-stack-based renderer
/// uses `fits_with_lookahead` directly. This function is kept for the
/// public API and Fill algorithm.
pub fn fits(doc: &Doc, width: usize, mode: Mode, config: &PrintConfig) -> bool {
    fits_with_lookahead::<dyn TextResolver>(doc, mode, &[], width as isize, config, None)
}

/// Check if a doc fits in the remaining width with symbol resolution
pub fn fits_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    width: usize,
    mode: Mode,
    config: &PrintConfig,
    resolver: &R,
) -> bool {
    fits_with_lookahead(doc, mode, &[], width as isize, config, Some(resolver))
}

/// Check if multiple docs fit sequentially in the remaining width
///
/// This is an optimization to avoid cloning docs just to check combined width.
/// Used by render_fill() to check if content + separator + next_content all fit.
pub(super) fn fits_multi<R: TextResolver + ?Sized>(
    docs: &[&Doc],
    width: usize,
    mode: Mode,
    config: &PrintConfig,
    resolver: Option<&R>,
) -> bool {
    if width == usize::MAX {
        return true;
    }

    let mut stack: SmallVec<[(&Doc, Mode); 16]> = SmallVec::new();
    let mut remaining_width = width as isize;

    // Push docs in reverse order (will be processed first-to-last)
    for doc in docs.iter().rev() {
        stack.push((doc, mode));
    }

    while let Some((current_doc, current_mode)) = stack.pop() {
        match current_doc {
            Doc::Text(t) => {
                let s = resolve_text(t, resolver);
                // If text contains a newline, content after newline is on a new line
                // and always "fits" for this check
                if s.contains('\n') {
                    return true;
                }
                remaining_width -= visual_width(s, config.tab_width) as isize;
                if remaining_width < 0 {
                    return false;
                }
            }

            Doc::Line(kind) => {
                match kind {
                    LineKind::Hard | LineKind::Literal => {
                        return true;
                    }
                    _ if current_mode == Mode::Break => {
                        return true;
                    }
                    LineKind::Soft => {
                        // Disappears in flat mode
                    }
                    LineKind::Normal => {
                        remaining_width -= 1;
                        if remaining_width < 0 {
                            return false;
                        }
                    }
                }
            }

            Doc::Group { contents, .. } => {
                // For conditional_group in flat mode, we should try all states,
                // but fits_multi is a simplified checker used by Fill algorithm
                // which doesn't need the full conditional_group logic.
                // Just check contents for now.
                stack.push((contents, current_mode));
            }

            Doc::IsolatedGroup { contents } => {
                stack.push((contents, current_mode));
            }

            Doc::Indent(inner) | Doc::Dedent(inner) => {
                // Indent/dedent don't affect width in fits() check
                // (indentation only matters at line breaks, which end fits() early)
                stack.push((inner, current_mode));
            }

            Doc::Align { contents, .. } | Doc::AlignSpaces { contents, .. } => {
                // Align/AlignSpaces don't affect width in fits() check
                stack.push((contents, current_mode));
            }

            Doc::IndentIfBreak { contents, .. } => {
                // IndentIfBreak doesn't affect width in fits() check
                // (like Indent, indentation only matters at line breaks)
                stack.push((contents, current_mode));
            }

            Doc::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if current_mode == Mode::Break {
                    break_doc
                } else {
                    flat_doc
                };
                stack.push((chosen, current_mode));
            }

            Doc::Concat(docs) => {
                for doc in docs.iter().rev() {
                    stack.push((doc, current_mode));
                }
            }

            Doc::Fill(parts) => {
                for doc in parts.iter().rev() {
                    stack.push((doc, current_mode));
                }
            }

            Doc::WithContext { doc, context } => {
                // Apply trailing_reserve for any doc type.
                remaining_width -= context.trailing_reserve as isize;
                if remaining_width < 0 {
                    return false;
                }
                stack.push((doc.as_ref(), current_mode));
            }

            Doc::LineSuffix(_) => {
                // LineSuffix content is NOT counted toward width.
            }

            Doc::LineSuffixBoundary => {
                // LineSuffixBoundary has no width effect.
            }

            Doc::BreakParent => {
                // BreakParent forces the enclosing group to break
                return false;
            }
        }
    }

    // Match prettier's fits() semantics: `while (remainingWidth >= 0)`
    // Content that uses exactly remaining width does fit
    remaining_width >= 0
}

//
// Width Calculation Helpers
//

/// Calculate available width for fitting check
///
/// This centralizes the width calculation logic used across TypeScript, CSS, and Svelte
/// formatters. It accounts for indentation and any trailing characters that will follow
/// the content being checked.
///
/// # Arguments
/// * `config` - Print configuration (contains print_width and tab_width)
/// * `indent_level` - Current indentation level
/// * `current_column` - Position on current line (0 if start of line)
/// * `trailing_chars` - Space to reserve for trailing punctuation (e.g., 1 for ";")
///
/// # Example
/// ```ignore
/// // Check if object fits after "const x = " on a line
/// let available = available_width(&config, 0, 10, 1); // 10 chars used, reserve 1 for ";"
/// ```
pub fn available_width(
    config: &PrintConfig,
    indent_level: usize,
    current_column: usize,
    trailing_chars: usize,
) -> usize {
    let indent_width = indent_level * config.tab_width;
    let used = indent_width.max(current_column) + trailing_chars;
    config.print_width.saturating_sub(used)
}

/// Check if a doc fits given the context
///
/// Convenience wrapper around `fits()` that handles width calculation.
///
/// # Arguments
/// * `doc` - Document to check
/// * `config` - Print configuration
/// * `indent_level` - Current indentation level
/// * `current_column` - Position on current line (0 if start of line)
/// * `trailing_chars` - Space to reserve for trailing punctuation
pub fn fits_at(
    doc: &Doc,
    config: &PrintConfig,
    indent_level: usize,
    current_column: usize,
    trailing_chars: usize,
) -> bool {
    let available = available_width(config, indent_level, current_column, trailing_chars);
    fits(doc, available, Mode::Flat, config)
}

//
// Utilities
//

/// Update position after rendering a text string, accounting for tab expansion.
///
/// If the string contains newlines, position is SET to the width of content
/// after the last newline (since newlines reset column position).
/// Otherwise, position is incremented by the string's visual width.
#[inline]
pub(super) fn update_pos_for_text(pos: &mut usize, s: &str, tab_width: usize) {
    if let Some(last_newline_pos) = s.rfind('\n') {
        // Reset position to visual width of content after last newline
        let after_newline = &s[last_newline_pos + 1..];
        *pos = visual_width(after_newline, tab_width);
    } else {
        // No newline - just increment by visual width
        *pos += visual_width(s, tab_width);
    }
}
