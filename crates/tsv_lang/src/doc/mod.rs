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
//! ## Architecture Note: Command Stack with Look-Ahead
//!
//! Like prettier's printer, this implementation uses a command stack approach.
//! When checking if a group fits (`fits()`), we pass the remaining command stack
//! so the algorithm can look ahead at what comes after the current group.
//!
//! This is critical for correct breaking decisions. For example:
//! ```text
//! (veryLongExpr || anotherLongExpr)!.method()
//! ```
//!
//! Without look-ahead, `fits()` would check if `(veryLongExpr || anotherLongExpr)`
//! fits and say "yes" (90 chars fits in 91 remaining). But the actual line includes
//! `!.method()` which pushes it over the limit.
//!
//! With look-ahead, `fits()` checks the group + everything after it, correctly
//! deciding to break.
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

pub mod arena;
mod arena_fits;
mod arena_render;
mod builders;
mod fits;
mod helpers;
mod render;
mod types;

// Re-export all public items

// Types
pub use types::{Doc, DocContext, DocText, GroupId, LineKind, Mode, TextResolver};

// Builders
pub use builders::{
    align, align_spaces, break_parent, concat, conditional_group, dedent, empty, fill, group,
    group_break, group_with_id, hardline, if_break, indent, indent_if_break, isolated_group, line,
    line_suffix, line_suffix_boundary, literalline, softline, symbol, text, text_owned,
    with_base_indent_override, with_context,
};

// Helpers
pub use helpers::{
    apply_indent_levels, braces, brackets, can_break, comma_hardline, comma_line, has_forced_break,
    indent_line, indent_softline, join, join_doc, join_trailing, parens, parens_break,
    remove_lines, trailing_comma, will_break, wrap,
};

// Fits
pub use fits::{available_width, fits, fits_at, fits_resolved};

// Render
pub use render::{
    print_doc, print_doc_at_column, print_doc_at_column_resolved, print_doc_resolved,
    print_doc_with_indent, print_doc_with_indent_resolved,
    print_doc_with_indent_resolved_preserve_whitespace,
};

// Arena render
pub use arena_render::{
    arena_print_doc, arena_print_doc_at_column, arena_print_doc_at_column_resolved,
    arena_print_doc_resolved, arena_print_doc_with_indent, arena_print_doc_with_indent_resolved,
    arena_print_doc_with_indent_resolved_preserve_whitespace,
};

// Arena fits
pub use arena_fits::arena_fits;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrintConfig;

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
        fn indent_str_width(indent: &str, tab_width: usize) -> usize {
            indent
                .chars()
                .map(|ch| if ch == '\t' { tab_width } else { 1 })
                .sum()
        }
        assert_eq!(indent_str_width("\t", 2), 2);
        assert_eq!(indent_str_width("\t", 4), 4);
        assert_eq!(indent_str_width("  ", 2), 2);
        assert_eq!(indent_str_width("\t\t", 2), 4);
    }

    //
    // Fill tests
    //

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
        // Continuation with +1 margin: remaining + 1 = 11, so "5, 6, 7, 8" (10) fits
        let config_no_offset = PrintConfig {
            indent: "\t",
            print_width: 12,
            tab_width: 2,
            base_indent_offset: 0,
            ..Default::default()
        };
        assert_eq!(
            print_doc(&doc, &config_no_offset),
            "1, 2, 3, 4,\n\t5, 6, 7, 8"
        );

        // With base_indent_offset=1: width = 12, tab_width = 2
        // base_indent_offset affects position calculation after newlines in render_single_doc
        // First line: same as without offset (base_indent only applies after newlines)
        // After newline: pos = 2 + 2 = 4 (local indent + base offset), remaining = 8
        // "5, 6, 7" = 7 chars fits, "5, 6, 7, 8" = 10 doesn't fit in 8
        // So: "5, 6, 7," then break, then "8"
        let config_with_offset = PrintConfig {
            indent: "\t",
            print_width: 12,
            tab_width: 2,
            base_indent_offset: 1,
            ..Default::default()
        };
        assert_eq!(
            print_doc(&doc, &config_with_offset),
            "1, 2, 3, 4,\n\t5, 6, 7,\n\t8"
        );
    }

    #[test]
    fn test_fill_wraps_last_item_at_101_chars() {
        // Reproduce the CSS animation-name bug:
        // When the line reaches exactly 101 chars (printWidth + 1), the last item
        // should wrap to a new line, but it's staying on the same line.
        //
        // Structure: indent (6 chars) + items with commas
        // Items: "a0000000000, a1111111111, ... a5555555555, " = 78 chars
        // Last item: "a6666666666666666" = 17 chars
        // Total: 6 + 78 + 17 = 101 chars → should wrap

        let items = vec![
            "a0000000000",
            "a1111111111",
            "a2222222222",
            "a3333333333",
            "a4444444444",
            "a5555555555",
            "a6666666666666666", // This long last item should wrap
        ];

        // Build fill doc: [item, ", ", item, ", ", ..., item]
        let mut parts = Vec::new();
        for (i, item) in items.iter().enumerate() {
            parts.push(text(*item));
            if i < items.len() - 1 {
                parts.push(concat(vec![text(","), line()]));
            }
        }

        let doc = fill(parts);

        let config = PrintConfig {
            indent: "\t",
            print_width: 100,
            tab_width: 2,
            base_indent_offset: 1, // Simulates CSS inside Svelte <style>
            ..Default::default()
        };

        // Simulate starting at indent position (3 tabs = 6 chars visual width)
        // This matches the real CSS case where we've already written:
        // <style>\n\tdiv {\n\t\tanimation-name:\n\t\t\t
        let start_column = 6; // 3 tabs × 2
        let indent_level = 3;
        let output = print_doc_with_indent(&doc, &config, start_column, indent_level);

        // Expected: last item wraps to new line
        // The long last item should NOT be on the same line as a5555555555
        assert!(
            !output.contains("a5555555555, a6666666666666666"),
            "Last item should wrap to new line, but found on same line as previous item"
        );

        // Should have the pattern: "a5555555555,\n\t\t\ta6666666666666666" (3 tabs for indent level 3)
        assert!(
            output.contains("a5555555555,\n\t\t\ta6666666666666666"),
            "Expected last item to be on its own line with proper indentation. Got:\n{}",
            output
        );
    }

    //
    // Helper function tests
    //

    #[test]
    fn test_join() {
        let docs = vec![text("a"), text("b"), text("c")];
        let doc = join(docs, ", ");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "a, b, c");
    }

    #[test]
    fn test_join_empty() {
        let docs: Vec<Doc> = vec![];
        let doc = join(docs, ", ");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "");
    }

    #[test]
    fn test_join_single() {
        let docs = vec![text("a")];
        let doc = join(docs, ", ");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "a");
    }

    #[test]
    fn test_join_doc_with_line() {
        // join_doc with line() separator
        let docs = vec![text("a"), text("b"), text("c")];
        let doc = group(join_doc(docs, line()));

        // Wide enough: fits on one line with spaces
        let config_wide = PrintConfig {
            print_width: 20,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_wide), "a b c");

        // Too narrow: breaks
        let config_narrow = PrintConfig {
            print_width: 3,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_narrow), "a\nb\nc");
    }

    #[test]
    fn test_join_doc_with_comma_line() {
        // join_doc with comma + line separator (common pattern)
        let docs = vec![text("item1"), text("item2"), text("item3")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(join_doc(docs, sep));

        // Wide enough
        let config_wide = PrintConfig {
            print_width: 30,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_wide), "item1, item2, item3");

        // Too narrow
        let config_narrow = PrintConfig {
            print_width: 10,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_narrow), "item1,\nitem2,\nitem3");
    }

    #[test]
    fn test_wrap() {
        let doc = wrap("(", text("content"), ")");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "(content)");
    }

    #[test]
    fn test_parens() {
        let doc = parens(text("x"));
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "(x)");
    }

    #[test]
    fn test_brackets() {
        let doc = brackets(text("0"));
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "[0]");
    }

    #[test]
    fn test_braces() {
        let doc = braces(text("a: 1"));
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "{a: 1}");
    }

    #[test]
    fn test_wrap_with_nested_content() {
        // Test wrap with more complex nested content
        let inner = concat(vec![text("a"), text(", "), text("b")]);
        let doc = brackets(inner);
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "[a, b]");
    }

    #[test]
    fn test_nested_wraps() {
        // Test nested wraps: { [x] }
        let doc = braces(concat(vec![text(" "), brackets(text("x")), text(" ")]));
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "{ [x] }");
    }

    //
    // Join trailing tests
    //

    #[test]
    fn test_join_trailing_flat() {
        // When flat, no trailing comma
        let docs = vec![text("a"), text("b"), text("c")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(join_trailing(docs, sep));
        let config = PrintConfig {
            print_width: 20,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a, b, c");
    }

    #[test]
    fn test_join_trailing_break() {
        // When breaking, adds trailing comma
        let docs = vec![text("a"), text("b"), text("c")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(join_trailing(docs, sep));
        let config = PrintConfig {
            print_width: 3,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a,\nb,\nc,");
    }

    #[test]
    fn test_join_trailing_empty() {
        let docs: Vec<Doc> = vec![];
        let sep = concat(vec![text(","), line()]);
        let doc = join_trailing(docs, sep);
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "");
    }

    #[test]
    fn test_join_trailing_single() {
        // Single item, no trailing comma in flat mode
        let docs = vec![text("a")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(join_trailing(docs, sep));
        let config = PrintConfig {
            print_width: 20,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a");
    }

    #[test]
    fn test_join_trailing_in_brackets() {
        // Common pattern: [a, b, c] or [\n  a,\n  b,\n  c,\n]
        let docs = vec![text("item1"), text("item2"), text("item3")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(concat(vec![
            text("["),
            indent(concat(vec![softline(), join_trailing(docs, sep)])),
            softline(),
            text("]"),
        ]));

        // Wide: fits on one line
        let wide = PrintConfig {
            print_width: 30,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &wide), "[item1, item2, item3]");

        // Narrow: breaks with trailing comma
        let narrow = PrintConfig {
            print_width: 15,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(
            print_doc(&doc, &narrow),
            "[\n  item1,\n  item2,\n  item3,\n]"
        );
    }

    //
    // Indent helper tests
    //

    #[test]
    fn test_indent_line() {
        // Content that doesn't fit should break with indent
        let doc = group(concat(vec![text("prefix"), indent_line(text("indented"))]));
        let config = PrintConfig {
            print_width: 10,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "prefix\n  indented");
    }

    #[test]
    fn test_indent_line_fits() {
        // When content fits, line becomes space
        let doc = group(concat(vec![text("a"), indent_line(text("b"))]));
        let config = PrintConfig {
            print_width: 20,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a b");
    }

    #[test]
    fn test_indent_softline_flat() {
        // Wide enough: softline disappears entirely
        let doc = group(concat(vec![text("a"), indent_softline(text("b"))]));
        let config = PrintConfig {
            print_width: 20,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "ab");
    }

    #[test]
    fn test_indent_softline_break() {
        // Too narrow: breaks with indent
        let doc = group(concat(vec![text("a"), indent_softline(text("b"))]));
        let config = PrintConfig {
            print_width: 1,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a\n  b");
    }

    /// Test documenting Doc size constraints.
    ///
    /// # Current State (32 bytes)
    /// Doc is 32 bytes after optimizing Group's expanded_states field:
    /// - Largest payload: Vec<Doc> at 24 bytes (Concat, Fill, Text variants)
    /// - Enum discriminant: 1 byte
    /// - Padding to 8-byte alignment: 7 bytes
    /// - Total: 32 bytes
    ///
    /// # Previous State (40 bytes)
    /// Before optimization, Group's `expanded_states: Option<Vec<Doc>>` (24 bytes)
    /// made Group the largest variant at 33 bytes → 40 bytes with padding.
    ///
    /// # Optimization Applied
    /// Changed `expanded_states: Option<Vec<Doc>>` → `expanded_states: Option<Box<Vec<Doc>>>`
    /// - Option<Box<Vec<Doc>>> is only 8 bytes (pointer or null)
    /// - Group is now 17 bytes → 24 with padding
    /// - Doc enum shrunk from 40 → 32 bytes (20% reduction)
    ///
    /// # Why Not 24 Bytes?
    /// The enum discriminant + padding adds 8 bytes to the largest 24-byte payload.
    /// Rust's enum layout: max(variant_payloads) + discriminant + padding.
    ///
    /// # Why SmallVec Doesn't Work for Concat
    /// SmallVec<[Doc; N]> stores N×32 bytes inline. Even SmallVec<[Doc; 1]> would be
    /// larger than the current Doc, bloating ALL variants.
    #[test]
    fn doc_size_constraints() {
        use super::types::Command;
        use smallvec::SmallVec;
        use std::mem::size_of;

        // Current sizes after optimization
        // Note: AlignSpaces variant added for Prettier-style tabs+spaces alignment
        assert_eq!(
            size_of::<Doc>(),
            32,
            "Doc size changed - review doc_size_constraints test"
        );
        assert_eq!(size_of::<Vec<Doc>>(), 24); // Largest payload (Concat, Fill)
        assert_eq!(size_of::<Box<Doc>>(), 8);
        assert_eq!(size_of::<Option<Box<Vec<Doc>>>>(), 8); // Optimized expanded_states

        // SmallVec analysis - still not viable even at 32 bytes
        assert_eq!(size_of::<SmallVec<[Doc; 4]>>(), 144); // 4×32 + overhead
        assert!(size_of::<SmallVec<[Doc; 1]>>() > size_of::<Doc>());

        // Command stack analysis (for potential SmallVec optimization)
        // Command { indent, mode, doc, base_indent_override, align_spaces } = 48 bytes
        //   - indent: usize (8), mode: Mode (1 + 7 padding), doc: &Doc (8),
        //   - base_indent_override: Option<usize> (16), align_spaces: usize (8)
        //
        // SmallVec viability for command stack:
        // - SmallVec<[Command; 16]> = 784 bytes on stack, avoids heap for depth ≤16
        // - Typical doc depth: 10-20 (functions, nested expressions)
        // - Trade-off: 784 bytes stack vs heap allocation
        //
        // Verdict: Less viable now with larger Command size. The heap allocation
        // for Vec is amortized across the entire render.
        assert_eq!(size_of::<Command>(), 48);
        assert_eq!(size_of::<SmallVec<[Command; 16]>>(), 784);
    }

    #[test]
    fn test_indent_softline_in_parens() {
        // Common pattern: arguments in parentheses
        let doc = group(concat(vec![
            text("fn("),
            indent_softline(text("arg1, arg2")),
            softline(),
            text(")"),
        ]));

        // Fits: flat
        let wide = PrintConfig {
            print_width: 30,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &wide), "fn(arg1, arg2)");

        // Doesn't fit: breaks
        let narrow = PrintConfig {
            print_width: 10,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &narrow), "fn(\n  arg1, arg2\n)");
    }

    //
    // IsolatedGroup tests
    //

    #[test]
    fn test_isolated_group_prevents_break_propagation() {
        // IsolatedGroup with hardline inside should not force parent to break
        let doc = group(concat(vec![
            text("fn("),
            isolated_group(concat(vec![text("a"), hardline(), text("b")])),
            text(")"),
        ]));
        let config = PrintConfig {
            print_width: 100,
            ..Default::default()
        };
        // Parent stays flat, internal hardline still breaks
        assert_eq!(print_doc(&doc, &config), "fn(a\nb)");
    }

    #[test]
    fn test_isolated_group_still_breaks_on_width() {
        // IsolatedGroup should still break if content doesn't fit
        let doc = group(concat(vec![
            text("fn("),
            indent_softline(isolated_group(text("verylongcontent"))),
            softline(),
            text(")"),
        ]));
        let config = PrintConfig {
            print_width: 10,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        // Outer group breaks because content doesn't fit
        assert!(print_doc(&doc, &config).contains('\n'));
    }

    #[test]
    fn test_will_break_false_for_isolated_group() {
        // will_break() should return false for IsolatedGroup
        let doc = isolated_group(concat(vec![text("a"), hardline(), text("b")]));
        assert!(!will_break(&doc));
    }

    #[test]
    fn test_isolated_group_with_softlines() {
        // IsolatedGroup with softlines should work normally
        let doc = group(concat(vec![
            text("outer("),
            isolated_group(group(concat(vec![
                text("inner("),
                indent_softline(text("content")),
                softline(),
                text(")"),
            ]))),
            text(")"),
        ]));
        let config = PrintConfig {
            print_width: 100,
            ..Default::default()
        };
        // Everything fits flat
        assert_eq!(print_doc(&doc, &config), "outer(inner(content))");
    }

    #[test]
    fn test_nested_isolated_groups() {
        // Nested IsolatedGroups should each isolate independently
        let doc = group(concat(vec![
            text("a("),
            isolated_group(concat(vec![
                text("b("),
                isolated_group(concat(vec![text("x"), hardline(), text("y")])),
                text(")"),
            ])),
            text(")"),
        ]));
        let config = PrintConfig {
            print_width: 100,
            ..Default::default()
        };
        // Both outer groups stay flat, inner hardline breaks
        assert_eq!(print_doc(&doc, &config), "a(b(x\ny))");
    }
}

#[cfg(test)]
mod arena_tests {
    use super::arena::DocArena;
    use super::*;
    use crate::PrintConfig;

    #[test]
    fn test_arena_simple_text() {
        let a = DocArena::new();
        let doc = a.text("hello");
        let config = PrintConfig::default();
        assert_eq!(arena_print_doc(&a, doc, &config), "hello");
    }

    #[test]
    fn test_arena_concat() {
        let a = DocArena::new();
        let doc = a.concat(vec![a.text("hello"), a.text(" "), a.text("world")]);
        let config = PrintConfig::default();
        assert_eq!(arena_print_doc(&a, doc, &config), "hello world");
    }

    #[test]
    fn test_arena_line_in_flat_mode_fits() {
        let a = DocArena::new();
        let doc = a.group(a.concat(vec![a.text("a"), a.line(), a.text("b")]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "a b");
    }

    #[test]
    fn test_arena_line_in_break_mode() {
        let a = DocArena::new();
        let doc = a.group(a.concat(vec![a.text("hello"), a.line(), a.text("world")]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 8,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "hello\nworld");
    }

    #[test]
    fn test_arena_hardline() {
        let a = DocArena::new();
        let doc = a.concat(vec![a.text("a"), a.hardline(), a.text("b")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 100,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "a\nb");
    }

    #[test]
    fn test_arena_softline() {
        let a = DocArena::new();
        let doc = a.group(a.concat(vec![a.text("a"), a.softline(), a.text("b")]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "ab");
    }

    #[test]
    fn test_arena_indent() {
        let a = DocArena::new();
        let inner = a.concat(vec![a.hardline(), a.text("child")]);
        let doc = a.concat(vec![a.text("parent"), a.indent(inner)]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 80,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "parent\n\tchild");
    }

    #[test]
    fn test_arena_group_with_indent() {
        let a = DocArena::new();
        let inner = a.concat(vec![a.line(), a.text("content")]);
        let indented = a.indent(inner);
        let doc = a.group(a.concat(vec![a.text("("), indented, a.line(), a.text(")")]));

        let config_wide = PrintConfig {
            indent: "  ",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config_wide), "( content )");

        let a2 = DocArena::new();
        let inner2 = a2.concat(vec![a2.line(), a2.text("content")]);
        let indented2 = a2.indent(inner2);
        let doc2 = a2.group(a2.concat(vec![a2.text("("), indented2, a2.line(), a2.text(")")]));

        let config_narrow = PrintConfig {
            indent: "  ",
            print_width: 8,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a2, doc2, &config_narrow), "(\n  content\n)");
    }

    #[test]
    fn test_arena_if_break() {
        let a = DocArena::new();
        let doc = a.group(a.concat(vec![
            a.text("("),
            a.if_break(a.text(",\n"), a.text(", ")),
            a.text(")"),
        ]));

        let config_wide = PrintConfig {
            indent: "\t",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config_wide), "(, )");
    }

    #[test]
    fn test_arena_dedent() {
        let a = DocArena::new();
        let inner = a.concat(vec![a.hardline(), a.text("back-to-level0")]);
        let dedented = a.dedent(inner);
        let doc = a.indent(a.concat(vec![
            a.text("level1"),
            a.hardline(),
            a.text("still-level1"),
            dedented,
        ]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 80,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(
            arena_print_doc(&a, doc, &config),
            "level1\n\tstill-level1\nback-to-level0"
        );
    }

    #[test]
    fn test_arena_fill_all_fit() {
        let a = DocArena::new();
        let doc = a.fill(vec![a.text("a"), a.line(), a.text("b"), a.line(), a.text("c")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "a b c");
    }

    #[test]
    fn test_arena_fill_greedy_packing() {
        let a = DocArena::new();
        let doc = a.fill(vec![
            a.text("aa"),
            a.line(),
            a.text("bb"),
            a.line(),
            a.text("cc"),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 6,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "aa bb\ncc");
    }

    #[test]
    fn test_arena_fill_long_comma_list() {
        let a = DocArena::new();
        let doc = a.fill(vec![
            a.text("aaaa"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("bbbb"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("cccc"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("dddd"),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 15,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "aaaa, bbbb,\ncccc, dddd");
    }

    #[test]
    fn test_arena_fill_with_base_indent_offset() {
        let a = DocArena::new();
        let doc = a.indent(a.fill(vec![
            a.text("1"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("2"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("3"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("4"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("5"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("6"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("7"),
            a.concat(vec![a.text(","), a.line()]),
            a.text("8"),
        ]));

        let config_no_offset = PrintConfig {
            indent: "\t",
            print_width: 12,
            tab_width: 2,
            base_indent_offset: 0,
            ..Default::default()
        };
        assert_eq!(
            arena_print_doc(&a, doc, &config_no_offset),
            "1, 2, 3, 4,\n\t5, 6, 7, 8"
        );

        let a2 = DocArena::new();
        let doc2 = a2.indent(a2.fill(vec![
            a2.text("1"),
            a2.concat(vec![a2.text(","), a2.line()]),
            a2.text("2"),
            a2.concat(vec![a2.text(","), a2.line()]),
            a2.text("3"),
            a2.concat(vec![a2.text(","), a2.line()]),
            a2.text("4"),
            a2.concat(vec![a2.text(","), a2.line()]),
            a2.text("5"),
            a2.concat(vec![a2.text(","), a2.line()]),
            a2.text("6"),
            a2.concat(vec![a2.text(","), a2.line()]),
            a2.text("7"),
            a2.concat(vec![a2.text(","), a2.line()]),
            a2.text("8"),
        ]));

        let config_with_offset = PrintConfig {
            indent: "\t",
            print_width: 12,
            tab_width: 2,
            base_indent_offset: 1,
            ..Default::default()
        };
        assert_eq!(
            arena_print_doc(&a2, doc2, &config_with_offset),
            "1, 2, 3, 4,\n\t5, 6, 7,\n\t8"
        );
    }

    #[test]
    fn test_arena_join() {
        let a = DocArena::new();
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let doc = a.join(docs, ", ");
        let config = PrintConfig::default();
        assert_eq!(arena_print_doc(&a, doc, &config), "a, b, c");
    }

    #[test]
    fn test_arena_join_empty() {
        let a = DocArena::new();
        let docs: Vec<_> = vec![];
        let doc = a.join(docs, ", ");
        let config = PrintConfig::default();
        assert_eq!(arena_print_doc(&a, doc, &config), "");
    }

    #[test]
    fn test_arena_join_doc_with_line() {
        let a = DocArena::new();
        let sep = a.line();
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let joined = a.join_doc(docs, sep);
        let doc = a.group(joined);

        let config_wide = PrintConfig {
            print_width: 20,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config_wide), "a b c");

        let a2 = DocArena::new();
        let sep2 = a2.line();
        let docs2 = vec![a2.text("a"), a2.text("b"), a2.text("c")];
        let joined2 = a2.join_doc(docs2, sep2);
        let doc2 = a2.group(joined2);

        let config_narrow = PrintConfig {
            print_width: 3,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a2, doc2, &config_narrow), "a\nb\nc");
    }

    #[test]
    fn test_arena_wrap() {
        let a = DocArena::new();
        let doc = a.wrap("(", a.text("content"), ")");
        let config = PrintConfig::default();
        assert_eq!(arena_print_doc(&a, doc, &config), "(content)");
    }

    #[test]
    fn test_arena_parens() {
        let a = DocArena::new();
        let doc = a.parens(a.text("x"));
        let config = PrintConfig::default();
        assert_eq!(arena_print_doc(&a, doc, &config), "(x)");
    }

    #[test]
    fn test_arena_brackets() {
        let a = DocArena::new();
        let doc = a.brackets(a.text("0"));
        let config = PrintConfig::default();
        assert_eq!(arena_print_doc(&a, doc, &config), "[0]");
    }

    #[test]
    fn test_arena_braces() {
        let a = DocArena::new();
        let doc = a.braces(a.text("a: 1"));
        let config = PrintConfig::default();
        assert_eq!(arena_print_doc(&a, doc, &config), "{a: 1}");
    }

    #[test]
    fn test_arena_join_trailing_flat() {
        let a = DocArena::new();
        let sep = a.concat(vec![a.text(","), a.line()]);
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let trailing = a.join_trailing(docs, sep);
        let doc = a.group(trailing);
        let config = PrintConfig {
            print_width: 20,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "a, b, c");
    }

    #[test]
    fn test_arena_join_trailing_break() {
        let a = DocArena::new();
        let sep = a.concat(vec![a.text(","), a.line()]);
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let trailing = a.join_trailing(docs, sep);
        let doc = a.group(trailing);
        let config = PrintConfig {
            print_width: 3,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "a,\nb,\nc,");
    }

    #[test]
    fn test_arena_indent_line() {
        let a = DocArena::new();
        let doc = a.group(a.concat(vec![
            a.text("prefix"),
            a.indent_line(a.text("indented")),
        ]));
        let config = PrintConfig {
            print_width: 10,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "prefix\n  indented");
    }

    #[test]
    fn test_arena_indent_softline_flat() {
        let a = DocArena::new();
        let doc = a.group(a.concat(vec![
            a.text("a"),
            a.indent_softline(a.text("b")),
        ]));
        let config = PrintConfig {
            print_width: 20,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "ab");
    }

    #[test]
    fn test_arena_isolated_group_prevents_break() {
        let a = DocArena::new();
        let inner = a.concat(vec![a.text("a"), a.hardline(), a.text("b")]);
        let iso = a.isolated_group(inner);
        let doc = a.group(a.concat(vec![a.text("fn("), iso, a.text(")")]));
        let config = PrintConfig {
            print_width: 100,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "fn(a\nb)");
    }

    #[test]
    fn test_arena_will_break_false_for_isolated() {
        let a = DocArena::new();
        let doc = a.isolated_group(a.concat(vec![a.text("a"), a.hardline(), a.text("b")]));
        assert!(!a.will_break(doc));
    }

    #[test]
    fn test_arena_nested_isolated_groups() {
        let a = DocArena::new();
        let inner_iso = a.isolated_group(a.concat(vec![a.text("x"), a.hardline(), a.text("y")]));
        let outer_iso = a.isolated_group(a.concat(vec![a.text("b("), inner_iso, a.text(")")]));
        let doc = a.group(a.concat(vec![a.text("a("), outer_iso, a.text(")")]));
        let config = PrintConfig {
            print_width: 100,
            ..Default::default()
        };
        assert_eq!(arena_print_doc(&a, doc, &config), "a(b(x\ny))");
    }

    #[test]
    fn test_arena_fill_wraps_last_item_at_101() {
        let a = DocArena::new();
        let items = vec![
            "a0000000000",
            "a1111111111",
            "a2222222222",
            "a3333333333",
            "a4444444444",
            "a5555555555",
            "a6666666666666666",
        ];

        let mut parts = Vec::new();
        for (i, item) in items.iter().enumerate() {
            parts.push(a.text(*item));
            if i < items.len() - 1 {
                parts.push(a.concat(vec![a.text(","), a.line()]));
            }
        }

        let doc = a.fill(parts);
        let config = PrintConfig {
            indent: "\t",
            print_width: 100,
            tab_width: 2,
            base_indent_offset: 1,
            ..Default::default()
        };

        let start_column = 6;
        let indent_level = 3;
        let output = arena_print_doc_with_indent(&a, doc, &config, start_column, indent_level);

        assert!(
            !output.contains("a5555555555, a6666666666666666"),
            "Last item should wrap"
        );
        assert!(
            output.contains("a5555555555,\n\t\t\ta6666666666666666"),
            "Expected last item on own line. Got:\n{}",
            output
        );
    }
}
