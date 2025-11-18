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
    Line { hard: bool, soft: bool },

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
    }
}

/// Create a soft line that disappears in flat mode (no space)
pub fn softline() -> Doc {
    Doc::Line {
        hard: false,
        soft: true,
    }
}

/// Create a hard line break (always breaks, never becomes a space)
pub fn hardline() -> Doc {
    Doc::Line {
        hard: true,
        soft: false,
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
    let mut stack: Vec<(&Doc, Mode, isize)> = vec![(doc, mode, 0)];
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

            Doc::Line { hard, soft } => {
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

            Doc::IfBreak { break_doc, flat_doc } => {
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
        }
    }

    remaining_width >= 0
}

/// Convert a Doc tree to a formatted string
///
/// This is the main entry point for rendering. It walks the Doc tree,
/// deciding when to break lines based on the `fits()` algorithm and
/// `print_width` configuration.
///
/// Based on prettier's rendering logic (prettier/doc.js).
pub fn print_doc(doc: &Doc, config: &PrintConfig) -> String {
    let mut output = String::new();
    let mut indent_level: usize = 0;
    let mut pos: usize = 0; // Current column position

    render_doc(doc, &mut output, &mut indent_level, &mut pos, Mode::Break, config);

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

        Doc::Line { hard, soft } => {
            if mode == Mode::Break || *hard {
                // Break: emit newline + indentation
                output.push('\n');
                write_indentation(output, *indent_level, config);
                *pos = indent_width(*indent_level, config);
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
            // Try flat mode first
            let remaining = config.print_width.saturating_sub(*pos);
            let mode = if fits(inner, remaining, Mode::Flat, config) {
                Mode::Flat
            } else {
                Mode::Break
            };
            render_doc(inner, output, indent_level, pos, mode, config);
        }

        Doc::IfBreak { break_doc, flat_doc } => {
            let chosen = if mode == Mode::Break { break_doc } else { flat_doc };
            render_doc(chosen, output, indent_level, pos, mode, config);
        }

        Doc::Concat(docs) => {
            for doc in docs {
                render_doc(doc, output, indent_level, pos, mode, config);
            }
        }
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
        };
        assert_eq!(print_doc(&doc, &config), "ab");
    }

    #[test]
    fn test_indent() {
        // Hardline must be INSIDE indent to get indented
        let doc = concat(vec![text("parent"), indent(concat(vec![hardline(), text("child")]))]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 80,
            tab_width: 2,
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
        };
        assert_eq!(print_doc(&doc, &config_wide), "( content )");

        // Doesn't fit: should break
        let config_narrow = PrintConfig {
            indent: "  ",
            print_width: 8,
            tab_width: 2,
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
            dedent(concat(vec![
                hardline(),
                text("back-to-level0"),
            ])),
        ]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 80,
            tab_width: 2,
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
}
