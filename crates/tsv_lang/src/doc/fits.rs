//! Width fitting algorithms for the doc builder

use crate::PrintConfig;
use smallvec::SmallVec;

use super::types::{Command, Doc, Mode, TextResolver, resolve_text};

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
    _config: &PrintConfig,
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
                remaining -= string_width(s) as isize;
                // Don't return early on negative - let the while condition catch it
            }

            Doc::Line { hard, soft, .. } => {
                if current_mode == Mode::Break || *hard {
                    // Line break found - rest fits on next line
                    return true;
                }
                // In flat mode: soft line disappears, regular line becomes space
                if !soft {
                    remaining -= 1;
                }
            }

            Doc::Group {
                contents,
                expanded_states,
            } => {
                // Match prettier: when in break mode with expanded states,
                // use the most expanded state (takes least space on current line)
                // See prettier printer.js lines 126-130
                let doc_to_check = if current_mode == Mode::Break {
                    if let Some(states) = expanded_states {
                        states.last().unwrap_or(contents)
                    } else {
                        contents
                    }
                } else {
                    contents
                };
                stack.push((doc_to_check, current_mode));
            }

            Doc::Indent(inner) | Doc::Dedent(inner) => {
                // Indent/dedent don't affect width in fits() check
                // (indentation only matters at line breaks, which end fits() early)
                stack.push((inner, current_mode));
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
    _config: &PrintConfig,
    resolver: Option<&R>,
) -> bool {
    if width == usize::MAX {
        return true;
    }

    let mut stack: SmallVec<[(&Doc, Mode, isize); 16]> = SmallVec::new();
    let mut remaining_width = width as isize;

    // Push docs in reverse order (will be processed first-to-last)
    for doc in docs.iter().rev() {
        stack.push((doc, mode, 0));
    }

    while let Some((current_doc, current_mode, indent_delta)) = stack.pop() {
        match current_doc {
            Doc::Text(t) => {
                let s = resolve_text(t, resolver);
                remaining_width -= string_width(s) as isize;
                if remaining_width < 0 {
                    return false;
                }
            }

            Doc::Line { hard, soft, .. } => {
                if current_mode == Mode::Break || *hard {
                    return true;
                }
                if !*soft {
                    remaining_width -= 1;
                    if remaining_width < 0 {
                        return false;
                    }
                }
            }

            Doc::Group { contents, .. } => {
                stack.push((contents, current_mode, indent_delta));
            }

            Doc::Indent(inner) => {
                stack.push((inner, current_mode, indent_delta + 1));
            }

            Doc::Dedent(inner) => {
                stack.push((inner, current_mode, indent_delta - 1));
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
                stack.push((chosen, current_mode, indent_delta));
            }

            Doc::Concat(docs) => {
                for doc in docs.iter().rev() {
                    stack.push((doc, current_mode, indent_delta));
                }
            }

            Doc::Fill(parts) => {
                for doc in parts.iter().rev() {
                    stack.push((doc, current_mode, indent_delta));
                }
            }

            Doc::WithContext { doc, context } => {
                // Apply trailing_reserve for any doc type.
                remaining_width -= context.trailing_reserve as isize;
                if remaining_width < 0 {
                    return false;
                }
                stack.push((doc.as_ref(), current_mode, indent_delta));
            }
        }
    }

    // Match prettier's fits() semantics: `while (remainingWidth >= 0)`
    // Content that uses exactly remaining width does fit
    remaining_width >= 0
}

// =============================================================================
// Width Calculation Helpers
// =============================================================================

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

// =============================================================================
// Utilities
// =============================================================================

/// Calculate visual width of a string
///
/// Uses a fast path for ASCII strings (O(1) via len()) since ~99% of
/// formatter output is ASCII. Falls back to char counting for non-ASCII.
///
/// TODO: For full Unicode support, should use a library like `unicode-width`.
/// For now, assumes non-ASCII multi-byte chars count as 1.
#[inline]
pub(super) fn string_width(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    }
}
