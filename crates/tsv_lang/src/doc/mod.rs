//! Arena-based document builder primitives for prettier-compatible formatting
//!
//! This module implements a declarative document builder architecture inspired by
//! prettier's doc builder (see prettier/doc.js). Formatters describe document
//! structure using primitives like `group()`, `line`, and `indent()` via the
//! `DocArena` allocator, and the rendering algorithm decides how to lay out
//! content based on the print width.
//!
//! ## Core Concepts
//!
//! - **DocArena**: Arena allocator that stores all doc nodes contiguously
//! - **DocId**: Lightweight handle (u32 index) into the arena
//! - **Mode**: Flat (try to fit on one line) vs Break (use line breaks)
//! - **arena_fits()**: Algorithm to check if a doc fits in remaining width
//! - **arena_print_doc()**: Convert doc tree to a final formatted string
//!
//! ## Architecture Note: Command Stack with Look-Ahead
//!
//! Like prettier's printer, this implementation uses a command stack approach.
//! When checking if a group fits, we pass the remaining command stack so the
//! algorithm can look ahead at what comes after the current group.

pub mod arena;
mod arena_fits;
mod arena_render;
mod render_config;
mod types;

// Types
pub use types::{DocContext, DocText, GroupId, LineKind, Mode, TextResolver};

// Arena render
pub use arena_render::{
    arena_print_doc, arena_print_doc_at_column, arena_print_doc_at_column_resolved,
    arena_print_doc_flat_resolved, arena_print_doc_resolved, arena_print_doc_with_indent,
    arena_print_doc_with_indent_resolved, arena_print_doc_with_indent_resolved_preserve_whitespace,
};

// Arena fits
pub use arena_fits::arena_fits;

use crate::{PRINT_WIDTH, TAB_WIDTH};

/// Calculate available width for fitting check
///
/// This centralizes the width calculation logic used across TypeScript, CSS, and Svelte
/// formatters. It accounts for indentation and any trailing characters that will follow
/// the content being checked.
///
/// Uses the hardcoded [`PRINT_WIDTH`] and [`TAB_WIDTH`].
///
/// # Arguments
/// * `indent_level` - Current indentation level
/// * `current_column` - Position on current line (0 if start of line)
/// * `trailing_chars` - Space to reserve for trailing punctuation (e.g., 1 for ";")
pub fn available_width(indent_level: usize, current_column: usize, trailing_chars: usize) -> usize {
    let indent_width = indent_level * TAB_WIDTH;
    let used = indent_width.max(current_column) + trailing_chars;
    PRINT_WIDTH.saturating_sub(used)
}

#[cfg(test)]
mod arena_tests {
    use super::arena::{DocArena, DocId};
    use super::arena_render::arena_print_doc_with_indent_and_render;
    use super::render_config::RenderConfig;
    use super::*;
    use crate::EmbedContext;

    /// Test helper: render with explicit width/tab/indent overrides and
    /// optional `base_indent_offset`. Wraps the internal
    /// [`arena_print_doc_with_indent_and_render`] for compactness.
    fn render_test(
        arena: &DocArena,
        doc: DocId,
        render: &RenderConfig,
        base_indent_offset: usize,
    ) -> String {
        let embed = EmbedContext {
            base_indent_offset,
            ..EmbedContext::default()
        };
        arena_print_doc_with_indent_and_render(arena, doc, &embed, 0, 0, render)
    }

    /// Test helper: render with default widths and the default embed context.
    fn render_default(arena: &DocArena, doc: DocId) -> String {
        arena_print_doc(arena, doc, &EmbedContext::default())
    }

    /// Test helper: render with explicit `print_width`, default tab/indent.
    fn render_pw(arena: &DocArena, doc: DocId, print_width: usize) -> String {
        let render = RenderConfig {
            print_width,
            ..RenderConfig::default()
        };
        render_test(arena, doc, &render, 0)
    }

    /// Test helper: render with explicit `print_width` and 2-space indent
    /// (matches the old `indent: "  ", tab_width: 2` test setup).
    fn render_pw_spaces(arena: &DocArena, doc: DocId, print_width: usize) -> String {
        let render = RenderConfig {
            print_width,
            tab_width: 2,
            indent: "  ",
        };
        render_test(arena, doc, &render, 0)
    }

    /// Test helper: render with explicit `print_width` and tab indent.
    fn render_pw_tab(arena: &DocArena, doc: DocId, print_width: usize) -> String {
        let render = RenderConfig {
            print_width,
            tab_width: 2,
            indent: "\t",
        };
        render_test(arena, doc, &render, 0)
    }

    #[test]
    fn test_arena_simple_text() {
        let a = DocArena::new(2);
        let doc = a.text("hello");
        assert_eq!(render_default(&a, doc), "hello");
    }

    #[test]
    fn test_arena_concat() {
        let a = DocArena::new(2);
        let doc = a.concat(&[a.text("hello"), a.text(" "), a.text("world")]);
        assert_eq!(render_default(&a, doc), "hello world");
    }

    #[test]
    fn test_arena_line_in_flat_mode_fits() {
        let a = DocArena::new(2);
        let doc = a.group(a.concat(&[a.text("a"), a.line(), a.text("b")]));
        assert_eq!(render_pw_tab(&a, doc, 10), "a b");
    }

    #[test]
    fn test_arena_line_in_break_mode() {
        let a = DocArena::new(2);
        let doc = a.group(a.concat(&[a.text("hello"), a.line(), a.text("world")]));
        assert_eq!(render_pw_tab(&a, doc, 8), "hello\nworld");
    }

    #[test]
    fn test_arena_hardline() {
        let a = DocArena::new(2);
        let doc = a.concat(&[a.text("a"), a.hardline(), a.text("b")]);
        assert_eq!(render_pw_tab(&a, doc, 100), "a\nb");
    }

    #[test]
    fn test_arena_softline() {
        let a = DocArena::new(2);
        let doc = a.group(a.concat(&[a.text("a"), a.softline(), a.text("b")]));
        assert_eq!(render_pw_tab(&a, doc, 10), "ab");
    }

    #[test]
    fn test_arena_indent() {
        let a = DocArena::new(2);
        let inner = a.concat(&[a.hardline(), a.text("child")]);
        let doc = a.concat(&[a.text("parent"), a.indent(inner)]);
        assert_eq!(render_pw_tab(&a, doc, 80), "parent\n\tchild");
    }

    #[test]
    fn test_arena_group_with_indent() {
        let a = DocArena::new(2);
        let inner = a.concat(&[a.line(), a.text("content")]);
        let indented = a.indent(inner);
        let doc = a.group(a.concat(&[a.text("("), indented, a.line(), a.text(")")]));

        assert_eq!(render_pw_spaces(&a, doc, 20), "( content )");

        let a2 = DocArena::new(2);
        let inner2 = a2.concat(&[a2.line(), a2.text("content")]);
        let indented2 = a2.indent(inner2);
        let doc2 = a2.group(a2.concat(&[a2.text("("), indented2, a2.line(), a2.text(")")]));

        assert_eq!(render_pw_spaces(&a2, doc2, 8), "(\n  content\n)");
    }

    #[test]
    fn test_arena_if_break() {
        let a = DocArena::new(2);
        let doc = a.group(a.concat(&[
            a.text("("),
            a.if_break(a.text(",\n"), a.text(", ")),
            a.text(")"),
        ]));

        assert_eq!(render_pw_tab(&a, doc, 20), "(, )");
    }

    #[test]
    fn test_arena_dedent() {
        let a = DocArena::new(2);
        let inner = a.concat(&[a.hardline(), a.text("back-to-level0")]);
        let dedented = a.dedent(inner);
        let doc = a.indent(a.concat(&[
            a.text("level1"),
            a.hardline(),
            a.text("still-level1"),
            dedented,
        ]));
        assert_eq!(
            render_pw_tab(&a, doc, 80),
            "level1\n\tstill-level1\nback-to-level0"
        );
    }

    #[test]
    fn test_arena_fill_all_fit() {
        let a = DocArena::new(2);
        let doc = a.fill(&[a.text("a"), a.line(), a.text("b"), a.line(), a.text("c")]);
        assert_eq!(render_pw_tab(&a, doc, 20), "a b c");
    }

    #[test]
    fn test_arena_fill_greedy_packing() {
        let a = DocArena::new(2);
        let doc = a.fill(&[a.text("aa"), a.line(), a.text("bb"), a.line(), a.text("cc")]);
        assert_eq!(render_pw_tab(&a, doc, 6), "aa bb\ncc");
    }

    #[test]
    fn test_arena_fill_long_comma_list() {
        let a = DocArena::new(2);
        let doc = a.fill(&[
            a.text("aaaa"),
            a.concat(&[a.text(","), a.line()]),
            a.text("bbbb"),
            a.concat(&[a.text(","), a.line()]),
            a.text("cccc"),
            a.concat(&[a.text(","), a.line()]),
            a.text("dddd"),
        ]);
        assert_eq!(render_pw_tab(&a, doc, 15), "aaaa, bbbb,\ncccc, dddd");
    }

    #[test]
    fn test_arena_fill_with_base_indent_offset() {
        let a = DocArena::new(2);
        let doc = a.indent(a.fill(&[
            a.text("1"),
            a.concat(&[a.text(","), a.line()]),
            a.text("2"),
            a.concat(&[a.text(","), a.line()]),
            a.text("3"),
            a.concat(&[a.text(","), a.line()]),
            a.text("4"),
            a.concat(&[a.text(","), a.line()]),
            a.text("5"),
            a.concat(&[a.text(","), a.line()]),
            a.text("6"),
            a.concat(&[a.text(","), a.line()]),
            a.text("7"),
            a.concat(&[a.text(","), a.line()]),
            a.text("8"),
        ]));

        let render = RenderConfig {
            print_width: 12,
            tab_width: 2,
            indent: "\t",
        };
        assert_eq!(
            render_test(&a, doc, &render, 0),
            "1, 2, 3, 4,\n\t5, 6, 7, 8"
        );

        let a2 = DocArena::new(2);
        let doc2 = a2.indent(a2.fill(&[
            a2.text("1"),
            a2.concat(&[a2.text(","), a2.line()]),
            a2.text("2"),
            a2.concat(&[a2.text(","), a2.line()]),
            a2.text("3"),
            a2.concat(&[a2.text(","), a2.line()]),
            a2.text("4"),
            a2.concat(&[a2.text(","), a2.line()]),
            a2.text("5"),
            a2.concat(&[a2.text(","), a2.line()]),
            a2.text("6"),
            a2.concat(&[a2.text(","), a2.line()]),
            a2.text("7"),
            a2.concat(&[a2.text(","), a2.line()]),
            a2.text("8"),
        ]));

        assert_eq!(
            render_test(&a2, doc2, &render, 1),
            "1, 2, 3, 4,\n\t5, 6, 7,\n\t8"
        );
    }

    #[test]
    fn test_arena_join() {
        let a = DocArena::new(2);
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let doc = a.join(docs, ", ");
        assert_eq!(render_default(&a, doc), "a, b, c");
    }

    #[test]
    fn test_arena_join_empty() {
        let a = DocArena::new(2);
        let docs: Vec<_> = vec![];
        let doc = a.join(docs, ", ");
        assert_eq!(render_default(&a, doc), "");
    }

    #[test]
    fn test_arena_join_doc_with_line() {
        let a = DocArena::new(2);
        let sep = a.line();
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let joined = a.join_doc(docs, sep);
        let doc = a.group(joined);

        assert_eq!(render_pw(&a, doc, 20), "a b c");

        let a2 = DocArena::new(2);
        let sep2 = a2.line();
        let docs2 = vec![a2.text("a"), a2.text("b"), a2.text("c")];
        let joined2 = a2.join_doc(docs2, sep2);
        let doc2 = a2.group(joined2);

        assert_eq!(render_pw(&a2, doc2, 3), "a\nb\nc");
    }

    #[test]
    fn test_arena_wrap() {
        let a = DocArena::new(2);
        let doc = a.wrap("(", a.text("content"), ")");
        assert_eq!(render_default(&a, doc), "(content)");
    }

    #[test]
    fn test_arena_parens() {
        let a = DocArena::new(2);
        let doc = a.parens(a.text("x"));
        assert_eq!(render_default(&a, doc), "(x)");
    }

    #[test]
    fn test_arena_brackets() {
        let a = DocArena::new(2);
        let doc = a.brackets(a.text("0"));
        assert_eq!(render_default(&a, doc), "[0]");
    }

    #[test]
    fn test_arena_braces() {
        let a = DocArena::new(2);
        let doc = a.braces(a.text("a: 1"));
        assert_eq!(render_default(&a, doc), "{a: 1}");
    }

    #[test]
    fn test_arena_join_trailing_flat() {
        let a = DocArena::new(2);
        let sep = a.concat(&[a.text(","), a.line()]);
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let trailing = a.join_trailing(docs, sep);
        let doc = a.group(trailing);
        assert_eq!(render_pw_spaces(&a, doc, 20), "a, b, c");
    }

    #[test]
    fn test_arena_join_trailing_break() {
        let a = DocArena::new(2);
        let sep = a.concat(&[a.text(","), a.line()]);
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let trailing = a.join_trailing(docs, sep);
        let doc = a.group(trailing);
        assert_eq!(render_pw_spaces(&a, doc, 3), "a,\nb,\nc,");
    }

    #[test]
    fn test_arena_indent_line() {
        let a = DocArena::new(2);
        let doc = a.group(a.concat(&[a.text("prefix"), a.indent_line(a.text("indented"))]));
        assert_eq!(render_pw_spaces(&a, doc, 10), "prefix\n  indented");
    }

    #[test]
    fn test_arena_indent_softline_flat() {
        let a = DocArena::new(2);
        let doc = a.group(a.concat(&[a.text("a"), a.indent_softline(a.text("b"))]));
        assert_eq!(render_pw_spaces(&a, doc, 20), "ab");
    }

    #[test]
    fn test_arena_isolated_group_prevents_break() {
        let a = DocArena::new(2);
        let inner = a.concat(&[a.text("a"), a.hardline(), a.text("b")]);
        let iso = a.isolated_group(inner);
        let doc = a.group(a.concat(&[a.text("fn("), iso, a.text(")")]));
        assert_eq!(render_pw(&a, doc, 100), "fn(a\nb)");
    }

    #[test]
    fn test_arena_will_break_false_for_isolated() {
        let a = DocArena::new(2);
        let doc = a.isolated_group(a.concat(&[a.text("a"), a.hardline(), a.text("b")]));
        assert!(!a.will_break(doc));
    }

    #[test]
    fn test_arena_nested_isolated_groups() {
        let a = DocArena::new(2);
        let inner_iso = a.isolated_group(a.concat(&[a.text("x"), a.hardline(), a.text("y")]));
        let outer_iso = a.isolated_group(a.concat(&[a.text("b("), inner_iso, a.text(")")]));
        let doc = a.group(a.concat(&[a.text("a("), outer_iso, a.text(")")]));
        assert_eq!(render_pw(&a, doc, 100), "a(b(x\ny))");
    }

    #[test]
    fn test_arena_fill_wraps_last_item_at_101() {
        let a = DocArena::new(2);
        let items = [
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
            parts.push(a.text(item));
            if i < items.len() - 1 {
                parts.push(a.concat(&[a.text(","), a.line()]));
            }
        }

        let doc = a.fill(&parts);
        let render = RenderConfig {
            print_width: 100,
            tab_width: 2,
            indent: "\t",
        };
        let embed = EmbedContext {
            base_indent_offset: 1,
            ..EmbedContext::default()
        };

        let start_column = 6;
        let indent_level = 3;
        let output = arena_print_doc_with_indent_and_render(
            &a,
            doc,
            &embed,
            start_column,
            indent_level,
            &render,
        );

        assert!(
            !output.contains("a5555555555, a6666666666666666"),
            "Last item should wrap"
        );
        assert!(
            output.contains("a5555555555,\n\t\t\ta6666666666666666"),
            "Expected last item on own line. Got:\n{output}"
        );
    }

    #[test]
    fn test_arena_join_single() {
        let a = DocArena::new(2);
        let docs = vec![a.text("a")];
        let doc = a.join(docs, ", ");
        assert_eq!(render_default(&a, doc), "a");
    }

    #[test]
    fn test_arena_join_doc_with_comma_line() {
        let a = DocArena::new(2);
        let sep = a.concat(&[a.text(","), a.line()]);
        let docs = vec![a.text("item1"), a.text("item2"), a.text("item3")];
        let joined = a.join_doc(docs, sep);
        let doc = a.group(joined);

        assert_eq!(render_pw(&a, doc, 30), "item1, item2, item3");

        let a2 = DocArena::new(2);
        let sep2 = a2.concat(&[a2.text(","), a2.line()]);
        let docs2 = vec![a2.text("item1"), a2.text("item2"), a2.text("item3")];
        let joined2 = a2.join_doc(docs2, sep2);
        let doc2 = a2.group(joined2);

        assert_eq!(render_pw(&a2, doc2, 10), "item1,\nitem2,\nitem3");
    }

    #[test]
    fn test_arena_join_trailing_empty() {
        let a = DocArena::new(2);
        let sep = a.concat(&[a.text(","), a.line()]);
        let docs: Vec<DocId> = vec![];
        let doc = a.join_trailing(docs, sep);
        assert_eq!(render_default(&a, doc), "");
    }

    #[test]
    fn test_arena_join_trailing_single() {
        let a = DocArena::new(2);
        let sep = a.concat(&[a.text(","), a.line()]);
        let docs = vec![a.text("a")];
        let trailing = a.join_trailing(docs, sep);
        let doc = a.group(trailing);
        assert_eq!(render_pw(&a, doc, 20), "a");
    }

    #[test]
    fn test_arena_join_trailing_in_brackets() {
        let a = DocArena::new(2);
        let sep = a.concat(&[a.text(","), a.line()]);
        let docs = vec![a.text("item1"), a.text("item2"), a.text("item3")];
        let trailing = a.join_trailing(docs, sep);
        let sl1 = a.softline();
        let inner = a.concat(&[sl1, trailing]);
        let indented = a.indent(inner);
        let sl2 = a.softline();
        let doc = a.group(a.concat(&[a.text("["), indented, sl2, a.text("]")]));

        assert_eq!(render_pw_spaces(&a, doc, 30), "[item1, item2, item3]");

        let a2 = DocArena::new(2);
        let sep2 = a2.concat(&[a2.text(","), a2.line()]);
        let docs2 = vec![a2.text("item1"), a2.text("item2"), a2.text("item3")];
        let trailing2 = a2.join_trailing(docs2, sep2);
        let sl1_2 = a2.softline();
        let inner2 = a2.concat(&[sl1_2, trailing2]);
        let indented2 = a2.indent(inner2);
        let sl2_2 = a2.softline();
        let doc2 = a2.group(a2.concat(&[a2.text("["), indented2, sl2_2, a2.text("]")]));

        assert_eq!(
            render_pw_spaces(&a2, doc2, 15),
            "[\n  item1,\n  item2,\n  item3,\n]"
        );
    }

    #[test]
    fn test_arena_fill_single_item() {
        let a = DocArena::new(2);
        let doc = a.fill(&[a.text("hello")]);
        assert_eq!(render_default(&a, doc), "hello");
    }

    #[test]
    fn test_arena_fill_two_items() {
        let a = DocArena::new(2);
        let doc = a.fill(&[a.text("a"), a.line(), a.text("b")]);
        assert_eq!(render_pw_tab(&a, doc, 10), "a b");
    }

    #[test]
    fn test_arena_fill_none_fit() {
        let a = DocArena::new(2);
        let doc = a.fill(&[
            a.text("verylongitem1"),
            a.line(),
            a.text("verylongitem2"),
            a.line(),
            a.text("verylongitem3"),
        ]);
        assert_eq!(
            render_pw_tab(&a, doc, 15),
            "verylongitem1\nverylongitem2\nverylongitem3"
        );
    }

    #[test]
    fn test_arena_fill_with_indent() {
        let a = DocArena::new(2);
        let doc = a.indent(a.fill(&[
            a.text("aaa"),
            a.line(),
            a.text("bbb"),
            a.line(),
            a.text("ccc"),
        ]));
        assert_eq!(render_pw_tab(&a, doc, 10), "aaa bbb\n\tccc");
    }

    #[test]
    fn test_arena_indent_softline_break() {
        let a = DocArena::new(2);
        let doc = a.group(a.concat(&[a.text("a"), a.indent_softline(a.text("b"))]));
        assert_eq!(render_pw_spaces(&a, doc, 1), "a\n  b");
    }

    #[test]
    fn test_arena_indent_softline_in_parens() {
        let a = DocArena::new(2);
        let sl = a.softline();
        let doc = a.group(a.concat(&[
            a.text("fn("),
            a.indent_softline(a.text("arg1, arg2")),
            sl,
            a.text(")"),
        ]));

        assert_eq!(render_pw_spaces(&a, doc, 30), "fn(arg1, arg2)");

        let a2 = DocArena::new(2);
        let sl2 = a2.softline();
        let doc2 = a2.group(a2.concat(&[
            a2.text("fn("),
            a2.indent_softline(a2.text("arg1, arg2")),
            sl2,
            a2.text(")"),
        ]));

        assert_eq!(render_pw_spaces(&a2, doc2, 10), "fn(\n  arg1, arg2\n)");
    }

    #[test]
    fn test_arena_indent_line_fits() {
        let a = DocArena::new(2);
        let doc = a.group(a.concat(&[a.text("a"), a.indent_line(a.text("b"))]));
        assert_eq!(render_pw_spaces(&a, doc, 20), "a b");
    }

    #[test]
    fn test_arena_isolated_group_still_breaks_on_width() {
        let a = DocArena::new(2);
        let iso = a.isolated_group(a.text("verylongcontent"));
        let doc = a.group(a.concat(&[
            a.text("fn("),
            a.indent_softline(iso),
            a.softline(),
            a.text(")"),
        ]));
        assert!(render_pw_spaces(&a, doc, 10).contains('\n'));
    }

    #[test]
    fn test_arena_isolated_group_with_softlines() {
        let a = DocArena::new(2);
        let inner_sl = a.softline();
        let inner_group = a.group(a.concat(&[
            a.text("inner("),
            a.indent_softline(a.text("content")),
            inner_sl,
            a.text(")"),
        ]));
        let iso = a.isolated_group(inner_group);
        let doc = a.group(a.concat(&[a.text("outer("), iso, a.text(")")]));
        assert_eq!(render_pw(&a, doc, 100), "outer(inner(content))");
    }

    #[test]
    fn test_arena_wrap_with_nested_content() {
        let a = DocArena::new(2);
        let inner = a.concat(&[a.text("a"), a.text(", "), a.text("b")]);
        let doc = a.brackets(inner);
        assert_eq!(render_default(&a, doc), "[a, b]");
    }

    #[test]
    fn test_arena_nested_wraps() {
        let a = DocArena::new(2);
        let inner = a.brackets(a.text("x"));
        let doc = a.braces(a.concat(&[a.text(" "), inner, a.text(" ")]));
        assert_eq!(render_default(&a, doc), "{ [x] }");
    }

    #[test]
    fn test_arena_line_in_break_mode_doesnt_fit() {
        let a = DocArena::new(2);
        let doc = a.group(a.concat(&[a.text("hello"), a.line(), a.text("world")]));
        assert_eq!(render_pw_tab(&a, doc, 8), "hello\nworld");
    }

    #[test]
    fn test_arena_tab_width_calculation() {
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
}
