//! Document builder primitives for prettier-compatible formatting
//!
//! This module implements a declarative document builder architecture inspired by
//! prettier's doc builder (see prettier/doc.js). Instead of imperatively deciding
//! when to break lines, formatters describe the document structure using primitives
//! like `group()`, `line`, and `indent()`, and let the rendering algorithm decide
//! how to lay out the content based on the print width.
//!
//! ## Core Concepts
//!
//! - **Doc**: An abstract document tree describing how content should be formatted
//! - **Mode**: Flat (try to fit on one line) vs Break (use line breaks)
//! - **fits()**: Algorithm to check if a doc fits in remaining width
//! - **print_doc()**: Convert a Doc tree to a final formatted string
//!
//! ## Example
//!
//! ```rust
//! use tsv_lang::doc::*;
//! use tsv_lang::PrintConfig;
//!
//! // Build a doc for an element with attributes
//! let doc = concat(vec![
//!     text("<"),
//!     text("Component"),
//!     indent(group(concat(vec![
//!         line(),
//!         text("prop1=\"value1\""),
//!         line(),
//!         text("prop2=\"value2\""),
//!         dedent(line()),
//!     ]))),
//!     text("/>"),
//! ]);
//!
//! // Render to string
//! let config = PrintConfig::default();
//! let output = print_doc(&doc, &config);
//! ```
//!
//! If the content fits within `print_width`, it renders on one line:
//! ```html
//! <Component prop1="value1" prop2="value2" />
//! ```
//!
//! If it doesn't fit, it breaks:
//! ```html
//! <Component
//!     prop1="value1"
//!     prop2="value2"
//! />
//! ```

use crate::PrintConfig;
use smallvec::SmallVec;

/// Document primitive - abstract representation of formatted output
#[derive(Debug, Clone)]
pub enum Doc {
    /// String literal - exact text to output
    Text(String),

    /// Line break - behavior depends on mode:
    /// - In Flat mode: becomes a space (unless `soft = true`)
    /// - In Break mode: becomes newline + indentation
    /// - `hard = true`: unconditional line break (always breaks)
    /// - `soft = true`: disappears in flat mode (no space)
    /// - `literal = true`: just newline, NO indentation (for blank lines)
    Line {
        hard: bool,
        soft: bool,
        literal: bool,
    },

    /// Increase indentation level for nested content
    Indent(Box<Doc>),

    /// Decrease indentation level
    Dedent(Box<Doc>),

    /// Try to fit content on one line; if doesn't fit, break ALL lines in group
    /// This is the key primitive for prettier's "all-or-nothing" breaking
    Group(Box<Doc>),

    /// Conditional rendering based on whether parent group breaks
    /// - If parent breaks: render `break_doc`
    /// - If parent fits: render `flat_doc`
    IfBreak {
        break_doc: Box<Doc>,
        flat_doc: Box<Doc>,
    },

    /// Sequence of docs - rendered one after another
    Concat(Vec<Doc>),

    /// Greedy line packing - fills each line with as much as fits
    ///
    /// Unlike Group (all-or-nothing breaking), Fill packs items left-to-right,
    /// breaking to a new line only when the next item wouldn't fit.
    ///
    /// Parts should alternate: [content, separator, content, separator, ...]
    /// Separators are typically `line()` or `softline()`.
    ///
    /// Example: `fill(vec![text("a"), line(), text("b"), line(), text("c")])`
    /// At width 5: "a b c" (all fit)
    /// At width 3: "a b\nc" (c doesn't fit with b)
    Fill(Vec<Doc>),
}

/// Rendering mode for a doc
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Try to fit on one line (soft lines become spaces)
    Flat,
    /// Use line breaks (soft lines become newlines)
    Break,
}

// =============================================================================
// Builder Helpers
// =============================================================================

/// Create a text doc
pub fn text(s: impl Into<String>) -> Doc {
    Doc::Text(s.into())
}

/// Create a soft line break (space if fits, newline if doesn't)
pub fn line() -> Doc {
    Doc::Line {
        hard: false,
        soft: false,
        literal: false,
    }
}

/// Create a soft line that disappears in flat mode (no space)
pub fn softline() -> Doc {
    Doc::Line {
        hard: false,
        soft: true,
        literal: false,
    }
}

/// Create a hard line break (always breaks, never becomes a space)
pub fn hardline() -> Doc {
    Doc::Line {
        hard: true,
        soft: false,
        literal: false,
    }
}

/// Create a literal line break (just newline, no indentation)
/// Used for blank line preservation where we want an empty line
pub fn literalline() -> Doc {
    Doc::Line {
        hard: true,
        soft: false,
        literal: true,
    }
}

/// Create a group (try to fit on one line, break all if doesn't fit)
pub fn group(doc: Doc) -> Doc {
    Doc::Group(Box::new(doc))
}

/// Increase indentation for nested doc
pub fn indent(doc: Doc) -> Doc {
    Doc::Indent(Box::new(doc))
}

/// Decrease indentation for doc
pub fn dedent(doc: Doc) -> Doc {
    Doc::Dedent(Box::new(doc))
}

/// Conditional rendering based on parent group breaking
pub fn if_break(break_doc: Doc, flat_doc: Doc) -> Doc {
    Doc::IfBreak {
        break_doc: Box::new(break_doc),
        flat_doc: Box::new(flat_doc),
    }
}

/// Concatenate multiple docs into a sequence
pub fn concat(docs: Vec<Doc>) -> Doc {
    Doc::Concat(docs)
}

/// Create a fill doc for greedy line packing
///
/// Fill packs as many items as possible on each line before breaking.
/// Parts should alternate between content and separators:
///
/// ```ignore
/// fill(vec![
///     text("item1"), line(),  // content, separator
///     text("item2"), line(),  // content, separator
///     text("item3"),          // final content (no trailing separator)
/// ])
/// ```
///
/// Separators should be Line variants (`line()`, `softline()`).
pub fn fill(parts: Vec<Doc>) -> Doc {
    Doc::Fill(parts)
}

// =============================================================================
// Analysis Utilities
// =============================================================================

/// Check if a doc will definitely break (contains hardline)
///
/// This is used for cascading breaks: if an inner doc will break,
/// outer groups should also break. For example, nested objects where
/// the inner object has a source newline after `{`.
///
/// Based on prettier's `willBreak()` utility.
pub fn will_break(doc: &Doc) -> bool {
    match doc {
        Doc::Text(_) => false,
        Doc::Line { hard, literal, .. } => *hard || *literal,
        Doc::Indent(inner) | Doc::Dedent(inner) | Doc::Group(inner) => will_break(inner),
        Doc::IfBreak { break_doc, .. } => will_break(break_doc),
        Doc::Concat(docs) | Doc::Fill(docs) => docs.iter().any(will_break),
    }
}

// =============================================================================
// Rendering Algorithm
// =============================================================================

/// Check if a doc fits in the remaining width
///
/// This simulates rendering the doc without actually building the string,
/// tracking the remaining width character-by-character. Returns `true` if
/// the doc fits, `false` otherwise.
///
/// Based on prettier's `fits()` algorithm (prettier/doc.js lines 850-930).
pub fn fits(doc: &Doc, width: usize, mode: Mode, _config: &PrintConfig) -> bool {
    if width == usize::MAX {
        return true; // Infinite width always fits
    }

    // Stack of (doc, mode, indent_delta) to process
    // indent_delta tracks indentation changes relative to current level
    // SmallVec avoids heap allocation for typical doc trees (<16 depth)
    let mut stack: SmallVec<[(&Doc, Mode, isize); 16]> = SmallVec::new();
    stack.push((doc, mode, 0));
    let mut remaining_width = width as isize;

    while let Some((current_doc, current_mode, indent_delta)) = stack.pop() {
        match current_doc {
            Doc::Text(s) => {
                // Deduct string width from remaining
                remaining_width -= string_width(s) as isize;
                if remaining_width < 0 {
                    return false; // Doesn't fit
                }
            }

            Doc::Line { hard, soft, .. } => {
                if current_mode == Mode::Break || *hard {
                    // Line break found - rest fits on next line
                    return true;
                }
                // In flat mode: soft line disappears, regular line becomes space
                if !soft {
                    remaining_width -= 1; // Space character
                    if remaining_width < 0 {
                        return false;
                    }
                }
            }

            Doc::Group(inner) => {
                // Groups propagate the mode (try flat first in actual rendering)
                stack.push((inner, current_mode, indent_delta));
            }

            Doc::Indent(inner) => {
                // Track indent delta but don't affect width in fits() check
                // (indentation only matters at line breaks, which end fits() early)
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
                // Process docs in reverse order (stack is LIFO)
                for doc in docs.iter().rev() {
                    stack.push((doc, current_mode, indent_delta));
                }
            }

            Doc::Fill(parts) => {
                // For fits() check, Fill behaves like Concat - just check all parts fit
                for doc in parts.iter().rev() {
                    stack.push((doc, current_mode, indent_delta));
                }
            }
        }
    }

    remaining_width >= 0
}

/// Check if multiple docs fit sequentially in the remaining width
///
/// This is an optimization to avoid cloning docs just to check combined width.
/// Used by render_fill() to check if content + separator + next_content all fit.
fn fits_multi(docs: &[&Doc], width: usize, mode: Mode, _config: &PrintConfig) -> bool {
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
            Doc::Text(s) => {
                remaining_width -= string_width(s) as isize;
                if remaining_width < 0 {
                    return false;
                }
            }

            Doc::Line { hard, soft, .. } => {
                if current_mode == Mode::Break || *hard {
                    return true;
                }
                if !soft {
                    remaining_width -= 1;
                    if remaining_width < 0 {
                        return false;
                    }
                }
            }

            Doc::Group(inner) => {
                stack.push((inner, current_mode, indent_delta));
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
        }
    }

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
// Doc Building Helpers
// =============================================================================

/// Build a doc from items with a separator between them
///
/// This is useful for building comma-separated or space-separated lists.
///
/// # Example
/// ```ignore
/// let docs = vec![text("a"), text("b"), text("c")];
/// let result = join(docs, ", ");
/// // Renders as: "a, b, c"
/// ```
pub fn join(docs: Vec<Doc>, separator: &str) -> Doc {
    if docs.is_empty() {
        return concat(vec![]);
    }
    let mut parts = Vec::with_capacity(docs.len() * 2 - 1);
    for (i, doc) in docs.into_iter().enumerate() {
        if i > 0 {
            parts.push(text(separator));
        }
        parts.push(doc);
    }
    concat(parts)
}

/// Convert a Doc tree to a formatted string (starting at column 0)
pub fn print_doc(doc: &Doc, config: &PrintConfig) -> String {
    print_doc_at_column(doc, config, 0)
}

/// Convert a Doc tree to a formatted string, starting at a specific column
///
/// Use this when the doc is being inserted into a line that already has content.
/// The `start_column` affects the width calculation for breaking decisions.
pub fn print_doc_at_column(doc: &Doc, config: &PrintConfig, start_column: usize) -> String {
    print_doc_with_indent(doc, config, start_column, 0)
}

/// Convert a Doc tree to a formatted string with both column and indent level specified
///
/// Use this when the doc is being inserted into content that already has both
/// column position and indentation context (e.g., Svelte template expressions).
pub fn print_doc_with_indent(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
) -> String {
    let mut output = String::new();
    let mut indent_level: usize = start_indent_level;
    let mut pos: usize = start_column;

    render_doc(
        doc,
        &mut output,
        &mut indent_level,
        &mut pos,
        Mode::Break,
        config,
    );

    output
}

/// Internal rendering implementation
fn render_doc(
    doc: &Doc,
    output: &mut String,
    indent_level: &mut usize,
    pos: &mut usize,
    mode: Mode,
    config: &PrintConfig,
) {
    match doc {
        Doc::Text(s) => {
            output.push_str(s);
            *pos += string_width(s);
        }

        Doc::Line {
            hard,
            soft,
            literal,
        } => {
            if mode == Mode::Break || *hard {
                // Break: emit newline
                output.push('\n');
                if *literal {
                    // Literal line: just newline, no indentation (for blank lines)
                    *pos = 0;
                } else {
                    // Normal line: newline + indentation
                    write_indentation(output, *indent_level, config);
                    // Account for base_indent_offset (e.g., Svelte wrapper indentation)
                    // This ensures fill/group width calculations are correct after newlines
                    let base_indent = config.base_indent_offset * config.tab_width;
                    *pos = indent_width(*indent_level, config) + base_indent;
                }
            } else {
                // Flat mode: soft line disappears, regular line becomes space
                if !*soft {
                    output.push(' ');
                    *pos += 1;
                }
            }
        }

        Doc::Indent(inner) => {
            *indent_level += 1;
            render_doc(inner, output, indent_level, pos, mode, config);
            *indent_level -= 1;
        }

        Doc::Dedent(inner) => {
            if *indent_level > 0 {
                *indent_level -= 1;
            }
            render_doc(inner, output, indent_level, pos, mode, config);
            *indent_level += 1;
        }

        Doc::Group(inner) => {
            // If inner will definitely break (contains hardline), use break mode
            // This enables cascading: nested objects with source newlines force outer to break
            if will_break(inner) {
                render_doc(inner, output, indent_level, pos, Mode::Break, config);
            } else {
                // Try flat mode first
                let remaining = config.print_width.saturating_sub(*pos);
                let chosen_mode = if fits(inner, remaining, Mode::Flat, config) {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                render_doc(inner, output, indent_level, pos, chosen_mode, config);
            }
        }

        Doc::IfBreak {
            break_doc,
            flat_doc,
        } => {
            let chosen = if mode == Mode::Break {
                break_doc
            } else {
                flat_doc
            };
            render_doc(chosen, output, indent_level, pos, mode, config);
        }

        Doc::Concat(docs) => {
            for doc in docs {
                render_doc(doc, output, indent_level, pos, mode, config);
            }
        }

        Doc::Fill(parts) => {
            render_fill(parts, output, indent_level, pos, config);
        }
    }
}

/// Render a fill doc using greedy line packing
///
/// This implements prettier's fill algorithm which packs as many items
/// as possible on each line before breaking to a new line.
///
/// The algorithm makes a three-way decision for each content+separator pair:
/// 1. Both current and next content fit → render both flat
/// 2. Only current content fits → render current flat, break, continue
/// 3. Neither fits → break current, break, continue
///
/// Based on prettier's fill algorithm (prettier/src/document/printer.js:360-450)
fn render_fill(
    parts: &[Doc],
    output: &mut String,
    indent_level: &mut usize,
    pos: &mut usize,
    config: &PrintConfig,
) {
    let mut offset = 0;

    while offset < parts.len() {
        let remaining = config.print_width.saturating_sub(*pos);
        let content = &parts[offset];

        // Check if current content fits in flat mode
        let content_fits = fits(content, remaining, Mode::Flat, config);

        // Case 1: Last item - just render it
        if offset + 1 >= parts.len() {
            let mode = if content_fits {
                Mode::Flat
            } else {
                Mode::Break
            };
            render_doc(content, output, indent_level, pos, mode, config);
            break;
        }

        let separator = &parts[offset + 1];

        // Case 2: Only content + separator left (no next content)
        if offset + 2 >= parts.len() {
            let mode = if content_fits {
                Mode::Flat
            } else {
                Mode::Break
            };
            render_doc(content, output, indent_level, pos, mode, config);
            render_doc(separator, output, indent_level, pos, mode, config);
            break;
        }

        // Case 3: Full three-way decision
        // Check if content + separator + next_content all fit together
        let next_content = &parts[offset + 2];
        // Use fits_multi to avoid cloning docs
        let both_fit = fits_multi(
            &[content, separator, next_content],
            remaining,
            Mode::Flat,
            config,
        );

        if both_fit {
            // Both fit: render content flat, separator flat
            render_doc(content, output, indent_level, pos, Mode::Flat, config);
            render_doc(separator, output, indent_level, pos, Mode::Flat, config);
        } else if content_fits {
            // Only first fits: render content flat, separator break (newline)
            render_doc(content, output, indent_level, pos, Mode::Flat, config);
            render_doc(separator, output, indent_level, pos, Mode::Break, config);
        } else {
            // Neither fits: render content break, separator break
            render_doc(content, output, indent_level, pos, Mode::Break, config);
            render_doc(separator, output, indent_level, pos, Mode::Break, config);
        }

        // Move past content and separator to next content
        offset += 2;
    }
}

// =============================================================================
// Utilities
// =============================================================================

/// Calculate visual width of a string
///
/// TODO: This is a simplified implementation that counts characters.
/// For full Unicode support, should use a library like `unicode-width`.
/// For now, assumes ASCII or that multi-byte chars count as 1.
fn string_width(s: &str) -> usize {
    s.chars().count()
}

/// Write indentation to output
fn write_indentation(output: &mut String, level: usize, config: &PrintConfig) {
    for _ in 0..level {
        output.push_str(config.indent);
    }
}

/// Calculate width of indentation
fn indent_width(level: usize, config: &PrintConfig) -> usize {
    level * indent_str_width(config.indent, config.tab_width)
}

/// Calculate visual width of indentation string
fn indent_str_width(indent: &str, tab_width: usize) -> usize {
    indent
        .chars()
        .map(|ch| if ch == '\t' { tab_width } else { 1 })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_text() {
        let doc = text("hello");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "hello");
    }

    #[test]
    fn test_concat() {
        let doc = concat(vec![text("hello"), text(" "), text("world")]);
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "hello world");
    }

    #[test]
    fn test_line_in_flat_mode_fits() {
        // Short content should fit on one line
        let doc = group(concat(vec![text("a"), line(), text("b")]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a b");
    }

    #[test]
    fn test_line_in_break_mode_doesnt_fit() {
        // Long content should break
        let doc = group(concat(vec![text("hello"), line(), text("world")]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 8, // Too narrow for "hello world"
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "hello\nworld");
    }

    #[test]
    fn test_hardline_always_breaks() {
        let doc = concat(vec![text("a"), hardline(), text("b")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 100,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a\nb");
    }

    #[test]
    fn test_softline_disappears_in_flat_mode() {
        let doc = group(concat(vec![text("a"), softline(), text("b")]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "ab");
    }

    #[test]
    fn test_indent() {
        // Hardline must be INSIDE indent to get indented
        let doc = concat(vec![
            text("parent"),
            indent(concat(vec![hardline(), text("child")])),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 80,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "parent\n\tchild");
    }

    #[test]
    fn test_group_with_indent() {
        let doc = group(concat(vec![
            text("("),
            indent(concat(vec![line(), text("content")])),
            line(),
            text(")"),
        ]));

        // Fits: should be flat
        let config_wide = PrintConfig {
            indent: "  ",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_wide), "( content )");

        // Doesn't fit: should break
        let config_narrow = PrintConfig {
            indent: "  ",
            print_width: 8,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_narrow), "(\n  content\n)");
    }

    #[test]
    fn test_if_break() {
        let doc = group(concat(vec![
            text("("),
            if_break(text(",\n"), text(", ")),
            text(")"),
        ]));

        // Fits: use flat_doc
        let config_wide = PrintConfig {
            indent: "\t",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_wide), "(, )");

        // Doesn't fit: use break_doc
        // Note: IfBreak needs parent group to break, so we need content that doesn't fit
        let doc_long = group(concat(vec![
            text("("),
            text("very long content that exceeds print width"),
            if_break(hardline(), text(" ")),
            text(")"),
        ]));

        let config_narrow = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        let result = print_doc(&doc_long, &config_narrow);
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_dedent() {
        // Dedent affects line breaks INSIDE the dedent block
        let doc = indent(concat(vec![
            text("level1"),
            hardline(),
            text("still-level1"),
            dedent(concat(vec![hardline(), text("back-to-level0")])),
        ]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 80,
            tab_width: 2,
            ..Default::default()
        };
        // First text at level 0, hardline at level 1, second text at level 1,
        // dedent's hardline at level 0, third text at level 0
        assert_eq!(
            print_doc(&doc, &config),
            "level1\n\tstill-level1\nback-to-level0"
        );
    }

    #[test]
    fn test_tab_width_calculation() {
        // Test that tabs are counted correctly for width
        assert_eq!(indent_str_width("\t", 2), 2);
        assert_eq!(indent_str_width("\t", 4), 4);
        assert_eq!(indent_str_width("  ", 2), 2);
        assert_eq!(indent_str_width("\t\t", 2), 4);
    }

    // ==========================================================================
    // Fill tests
    // ==========================================================================

    #[test]
    fn test_fill_all_fit() {
        // All items fit on one line
        let doc = fill(vec![text("a"), line(), text("b"), line(), text("c")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a b c");
    }

    #[test]
    fn test_fill_greedy_packing() {
        // Items should pack greedily: "a b" fits, then "c" on next line
        let doc = fill(vec![text("aa"), line(), text("bb"), line(), text("cc")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 6, // "aa bb" = 5 chars, fits; "aa bb cc" = 8, doesn't fit
            tab_width: 2,
            ..Default::default()
        };
        // "aa bb" fits (5 chars), but "aa bb cc" (8 chars) doesn't
        // So: "aa bb\ncc"
        assert_eq!(print_doc(&doc, &config), "aa bb\ncc");
    }

    #[test]
    fn test_fill_single_item() {
        let doc = fill(vec![text("hello")]);
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "hello");
    }

    #[test]
    fn test_fill_two_items() {
        let doc = fill(vec![text("a"), line(), text("b")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a b");
    }

    #[test]
    fn test_fill_with_indent() {
        // Fill inside indent should have indented continuation lines
        let doc = indent(fill(vec![
            text("aaa"),
            line(),
            text("bbb"),
            line(),
            text("ccc"),
        ]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 10, // "\taaa bbb" = 9 chars (tab=2), fits; "\taaa bbb ccc" = 13, doesn't
            tab_width: 2,
            ..Default::default()
        };
        // At indent level 1, "aaa bbb" fits, "ccc" wraps
        assert_eq!(print_doc(&doc, &config), "aaa bbb\n\tccc");
    }

    #[test]
    fn test_fill_comma_separated() {
        // Simulate CSS comma-separated list: item1, item2, item3
        let doc = fill(vec![
            text("item1"),
            text(", "),
            text("item2"),
            text(", "),
            text("item3"),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "item1, item2, item3");
    }

    #[test]
    fn test_fill_long_comma_list_wraps() {
        // Long list should wrap with greedy packing
        let doc = fill(vec![
            text("aaaa"),
            concat(vec![text(","), line()]),
            text("bbbb"),
            concat(vec![text(","), line()]),
            text("cccc"),
            concat(vec![text(","), line()]),
            text("dddd"),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 15, // "aaaa, bbbb" = 10, fits; add ", cccc" = 17, doesn't
            tab_width: 2,
            ..Default::default()
        };
        // "aaaa, bbbb" fits (10), "aaaa, bbbb, cccc" (17) doesn't
        // So: "aaaa, bbbb,\ncccc, dddd"
        assert_eq!(print_doc(&doc, &config), "aaaa, bbbb,\ncccc, dddd");
    }

    #[test]
    fn test_fill_none_fit() {
        // Each item is too long to fit with another
        let doc = fill(vec![
            text("verylongitem1"),
            line(),
            text("verylongitem2"),
            line(),
            text("verylongitem3"),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 15,
            tab_width: 2,
            ..Default::default()
        };
        // Each item alone fits, but no two items fit together
        assert_eq!(
            print_doc(&doc, &config),
            "verylongitem1\nverylongitem2\nverylongitem3"
        );
    }

    #[test]
    fn test_fill_with_base_indent_offset() {
        // Test that base_indent_offset affects width calculations after newlines
        // Simulates TypeScript array inside Svelte <script> tag
        //
        // The base_indent_offset only affects position calculation AFTER a newline.
        // The first line is unaffected since we start at column 0.
        let doc = indent(fill(vec![
            text("1"),
            concat(vec![text(","), line()]),
            text("2"),
            concat(vec![text(","), line()]),
            text("3"),
            concat(vec![text(","), line()]),
            text("4"),
            concat(vec![text(","), line()]),
            text("5"),
            concat(vec![text(","), line()]),
            text("6"),
            concat(vec![text(","), line()]),
            text("7"),
            concat(vec![text(","), line()]),
            text("8"),
        ]));

        // Without base_indent_offset: width = 12, tab_width = 2
        // First line: fills until "1, 2, 3, 4," (10 chars), break after 4
        // After newline: pos = 2 (just local indent), remaining = 10
        // Continuation: "5, 6, 7, 8" = 10 chars, fits exactly
        let config_no_offset = PrintConfig {
            indent: "\t",
            print_width: 12,
            tab_width: 2,
            base_indent_offset: 0,
        };
        assert_eq!(
            print_doc(&doc, &config_no_offset),
            "1, 2, 3, 4,\n\t5, 6, 7, 8"
        );

        // With base_indent_offset=1: width = 12, tab_width = 2
        // First line: same as above, fills to "1, 2, 3, 4,"
        // After newline: pos = 2 + 2 = 4 (local + base), remaining = 8
        // Continuation: "5, 6, 7" = 7 chars fits, "5, 6, 7, 8" = 10 doesn't fit
        // So: "5, 6, 7," then break, then "8"
        let config_with_offset = PrintConfig {
            indent: "\t",
            print_width: 12,
            tab_width: 2,
            base_indent_offset: 1,
        };
        assert_eq!(
            print_doc(&doc, &config_with_offset),
            "1, 2, 3, 4,\n\t5, 6, 7,\n\t8"
        );
    }
}
