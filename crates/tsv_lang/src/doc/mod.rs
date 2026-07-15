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
mod arena_render_fill;
mod render_config;
#[cfg(feature = "swallow_check")]
pub mod swallow;
mod types;

// Types
pub use types::{
    CachedWidth, DocContext, DocText, GroupId, LineKind, Mode, PoolSpan, SourceTextResolver,
    TextResolver,
};

/// Stack buffer for assembling a node's doc parts before handing them to
/// `DocArena::concat` / `fill`. Language printers build one such `Vec<DocId>` per
/// AST node — collectively a top format-phase allocation source — yet most nodes
/// have only a handful of parts, so the common case stays on the stack and only
/// larger nodes spill. Shared by the TS chain / binary-operator printers and the
/// Svelte template printer; `DocId` is `Copy` and 4 bytes → 32-byte inline buffer.
pub type DocBuf = smallvec::SmallVec<[arena::DocId; 8]>;

// Diagnostic: line-comment swallow check (opt-in, render-time; `swallow_check` feature)
#[cfg(feature = "swallow_check")]
pub use swallow::{SwallowReport, set_swallow_check, swallow_check_enabled, take_swallow_reports};

// Arena render
pub use arena_render::{
    arena_measure_doc_flat_resolved, arena_print_doc, arena_print_doc_at_column,
    arena_print_doc_with_indent, arena_print_doc_with_indent_resolved,
    arena_print_doc_with_indent_resolved_into,
    arena_print_doc_with_indent_resolved_preserve_whitespace,
    arena_print_doc_with_indent_resolved_preserve_whitespace_into,
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

    /// Test helper: render with explicit width/indent overrides and
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

    /// Test helper: render with explicit `print_width`, default indent.
    fn render_pw(arena: &DocArena, doc: DocId, print_width: usize) -> String {
        let render = RenderConfig {
            print_width,
            ..RenderConfig::default()
        };
        render_test(arena, doc, &render, 0)
    }

    /// Test helper: render with explicit `print_width` and 2-space indent
    /// (matches the old `indent: "  "` test setup).
    fn render_pw_spaces(arena: &DocArena, doc: DocId, print_width: usize) -> String {
        let render = RenderConfig {
            print_width,
            indent: "  ",
            ..RenderConfig::default()
        };
        render_test(arena, doc, &render, 0)
    }

    /// Test helper: render with explicit `print_width` and tab indent.
    fn render_pw_tab(arena: &DocArena, doc: DocId, print_width: usize) -> String {
        let render = RenderConfig {
            print_width,
            indent: "\t",
            ..RenderConfig::default()
        };
        render_test(arena, doc, &render, 0)
    }

    #[test]
    fn test_available_width() {
        // PRINT_WIDTH = 100, TAB_WIDTH = 2.
        assert_eq!(available_width(0, 0, 0), 100);
        // indent 2 levels (2*2=4) + 1 trailing char reserved.
        assert_eq!(available_width(2, 0, 1), 100 - 4 - 1);
        // current_column (50) dominates the indent width (1*2=2).
        assert_eq!(available_width(1, 50, 0), 50);
        // saturating floor: never underflows below 0.
        assert_eq!(available_width(0, 200, 0), 0);
    }

    #[test]
    fn test_text_width_precompute_clamps_below_sentinels() {
        use super::arena::DocNode;
        use super::types::CachedWidth;

        let cached = |s: &str| {
            let arena = DocArena::new();
            let id = arena.text_pooled(s);
            let nodes = arena.borrow_nodes();
            let DocNode::Text(t) = &nodes[id.index()] else {
                panic!("expected text node");
            };
            t.cached_width()
        };

        const MAX_CACHEABLE: u16 = u16::MAX - 2; // one below TEXT_WIDTH_NOT_COMPUTED

        // Width 65,533 (32,766 CJK × 2 + 1): the widest exactly-cacheable text.
        assert_eq!(
            cached(&("中".repeat(32_766) + "x")),
            CachedWidth::Width(MAX_CACHEABLE)
        );
        // Width 65,534 would alias TEXT_WIDTH_NOT_COMPUTED; must clamp.
        assert_eq!(
            cached(&"中".repeat(32_767)),
            CachedWidth::Width(MAX_CACHEABLE)
        );
        // Width 65,536+ would wrap under a bare `as u16` (→ "always fits");
        // must clamp instead.
        assert_eq!(
            cached(&"中".repeat(40_000)),
            CachedWidth::Width(MAX_CACHEABLE)
        );
        // Newline-bearing text is flagged, never measured.
        assert_eq!(cached("中\n中"), CachedWidth::HasNewline);
    }

    // `MultilineText::first_width` precomputes the first line's visual width with
    // the same `.min(TEXT_WIDTH_NOT_COMPUTED - 1)` clamp as `pooled_text_width`
    // (arena.rs `multiline_text`). No corpus reaches the clamp — it needs a
    // ~65k-column first line — so this is the only gate over that arm (mutation
    // survivor: the `- 1` in the clamp).
    #[test]
    fn test_multiline_first_width_precompute_clamps_below_sentinels() {
        use super::arena::DocNode;

        let a = DocArena::new();
        let first_width = |s: &str| {
            let id = a.multiline_text(s);
            let nodes = a.borrow_nodes();
            let DocNode::MultilineText { first_width, .. } = &nodes[id.index()] else {
                panic!("expected multiline-text node");
            };
            *first_width
        };

        const MAX_CACHEABLE: u16 = u16::MAX - 2; // one below TEXT_WIDTH_NOT_COMPUTED

        // Ordinary first lines carry their exact visual width (tabs = TAB_WIDTH).
        assert_eq!(first_width("abcd\ntail"), 4);
        assert_eq!(first_width("a\tb\ntail"), 4);
        // First line 65,533 cols (32,766 CJK × 2 + 1): the widest exactly cacheable.
        assert_eq!(
            first_width(&("中".repeat(32_766) + "x\ntail")),
            MAX_CACHEABLE
        );
        // First line 65,534 cols would alias TEXT_WIDTH_NOT_COMPUTED; must clamp.
        assert_eq!(
            first_width(&("中".repeat(32_767) + "\ntail")),
            MAX_CACHEABLE
        );
        // Only the first line is measured; a wide continuation line is irrelevant.
        assert_eq!(first_width(&format!("ok\n{}", "中".repeat(40_000))), 2);
    }

    #[test]
    fn test_static_text_width_cached_via_static_cache() {
        use super::arena::DocNode;
        use super::types::CachedWidth;

        let cached_static = |arena: &DocArena, s: &'static str| {
            let id = arena.text(s);
            let nodes = arena.borrow_nodes();
            let DocNode::Text(t) = &nodes[id.index()] else {
                panic!("expected text node");
            };
            t.cached_width()
        };

        let mut a = DocArena::new();
        // Statics always carry a real cached width (never NOT_COMPUTED) —
        // first sighting (cache miss) and repeat (cache hit) agree.
        assert_eq!(cached_static(&a, ",="), CachedWidth::Width(2));
        assert_eq!(cached_static(&a, ",="), CachedWidth::Width(2));
        // A newline-bearing static routes to the sentinel through the same
        // cache, exactly like pooled text.
        assert_eq!(cached_static(&a, "a\nb"), CachedWidth::HasNewline);
        // The cache survives reset() (entries key on 'static addresses):
        // the next document still reads real widths, including the sentinel.
        a.reset();
        assert_eq!(cached_static(&a, ",="), CachedWidth::Width(2));
        assert_eq!(cached_static(&a, "a\nb"), CachedWidth::HasNewline);
        // The empty() fast path bypasses the cache with a constant width 0,
        // which must agree with what the cache would compute.
        assert_eq!(cached_static(&a, ""), CachedWidth::Width(0));
    }

    #[test]
    fn test_static_text_node_interned_per_document() {
        let mut a = DocArena::new();

        // Repeated statics within one document share one node.
        let comma_1 = a.text(",");
        let comma_2 = a.text(",");
        assert_eq!(comma_1, comma_2);
        // A different static gets its own node.
        let semi = a.text(";");
        assert_ne!(comma_1, semi);
        // empty() interns through its dedicated cell.
        let empty_1 = a.empty();
        let empty_2 = a.empty();
        assert_eq!(empty_1, empty_2);
        let node_count = a.borrow_nodes().len();
        assert_eq!(node_count, 3); // ",", ";", ""

        // reset() invalidates every interned node (ids restart at 0): the next
        // document re-allocs rather than returning a prior document's id, and
        // interning resumes within it.
        a.reset();
        let comma_3 = a.text(",");
        let empty_3 = a.empty();
        assert_eq!(comma_3.index(), 0);
        assert_eq!(empty_3.index(), 1);
        assert_eq!(a.text(","), comma_3);
        assert_eq!(a.empty(), empty_3);
    }

    #[test]
    fn test_singleton_nodes_interned_per_document() {
        let mut a = DocArena::new();

        // Each Line kind shares one node per document; kinds stay distinct.
        let normal = a.line();
        assert_eq!(a.line(), normal);
        let soft = a.softline();
        let hard = a.hardline();
        let literal = a.literalline();
        assert_ne!(normal, soft);
        assert_ne!(soft, hard);
        assert_ne!(hard, literal);
        assert_eq!(a.softline(), soft);
        assert_eq!(a.hardline(), hard);
        assert_eq!(a.literalline(), literal);
        // LineSuffixBoundary and BreakParent intern through their own cells.
        let lsb = a.line_suffix_boundary();
        assert_eq!(a.line_suffix_boundary(), lsb);
        let bp = a.break_parent();
        assert_eq!(a.break_parent(), bp);
        assert_eq!(a.borrow_nodes().len(), 6); // 4 line kinds + LSB + BreakParent

        // reset() invalidates every interned singleton (ids restart at 0):
        // the next document re-allocs rather than returning a prior
        // document's id, and interning resumes within it.
        a.reset();
        let normal_2 = a.line();
        let lsb_2 = a.line_suffix_boundary();
        let bp_2 = a.break_parent();
        assert_eq!(normal_2.index(), 0);
        assert_eq!(lsb_2.index(), 1);
        assert_eq!(bp_2.index(), 2);
        assert_eq!(a.line(), normal_2);
        assert_eq!(a.line_suffix_boundary(), lsb_2);
        assert_eq!(a.break_parent(), bp_2);
    }

    #[test]
    fn test_symbol_node_interned_per_document() {
        let mut a = DocArena::new();

        // Repeated symbol ids within one document share one node; distinct
        // ids stay distinct (including sparse ids beyond the table's growth).
        let sym_5 = a.symbol(5);
        assert_eq!(a.symbol(5), sym_5);
        let sym_0 = a.symbol(0);
        assert_ne!(sym_5, sym_0);
        assert_eq!(a.symbol(0), sym_0);
        assert_eq!(a.borrow_nodes().len(), 2); // ids 5 and 0

        // reset() invalidates every interned symbol node (ids restart at 0):
        // the next document re-allocs rather than returning a prior
        // document's id, and interning resumes within it.
        a.reset();
        let sym_5_2 = a.symbol(5);
        assert_eq!(sym_5_2.index(), 0);
        assert_eq!(a.symbol(5), sym_5_2);
        assert_ne!(a.symbol(9), sym_5_2);
    }

    #[test]
    fn test_arena_simple_text() {
        let a = DocArena::new();
        let doc = a.text("hello");
        assert_eq!(render_default(&a, doc), "hello");
    }

    #[test]
    fn test_arena_concat() {
        let a = DocArena::new();
        let doc = a.concat(&[a.text("hello"), a.text(" "), a.text("world")]);
        assert_eq!(render_default(&a, doc), "hello world");
    }

    #[test]
    fn test_arena_line_in_flat_mode_fits() {
        let a = DocArena::new();
        let doc = a.group(a.concat(&[a.text("a"), a.line(), a.text("b")]));
        assert_eq!(render_pw_tab(&a, doc, 10), "a b");
    }

    #[test]
    fn test_arena_line_in_break_mode() {
        let a = DocArena::new();
        let doc = a.group(a.concat(&[a.text("hello"), a.line(), a.text("world")]));
        assert_eq!(render_pw_tab(&a, doc, 8), "hello\nworld");
    }

    #[test]
    fn test_arena_hardline() {
        let a = DocArena::new();
        let doc = a.concat(&[a.text("a"), a.hardline(), a.text("b")]);
        assert_eq!(render_pw_tab(&a, doc, 100), "a\nb");
    }

    #[test]
    fn test_arena_softline() {
        let a = DocArena::new();
        let doc = a.group(a.concat(&[a.text("a"), a.softline(), a.text("b")]));
        assert_eq!(render_pw_tab(&a, doc, 10), "ab");
    }

    #[test]
    fn test_arena_indent() {
        let a = DocArena::new();
        let inner = a.concat(&[a.hardline(), a.text("child")]);
        let doc = a.concat(&[a.text("parent"), a.indent(inner)]);
        assert_eq!(render_pw_tab(&a, doc, 80), "parent\n\tchild");
    }

    #[test]
    fn test_arena_group_with_indent() {
        let a = DocArena::new();
        let inner = a.concat(&[a.line(), a.text("content")]);
        let indented = a.indent(inner);
        let doc = a.group(a.concat(&[a.text("("), indented, a.line(), a.text(")")]));

        assert_eq!(render_pw_spaces(&a, doc, 20), "( content )");

        let a2 = DocArena::new();
        let inner2 = a2.concat(&[a2.line(), a2.text("content")]);
        let indented2 = a2.indent(inner2);
        let doc2 = a2.group(a2.concat(&[a2.text("("), indented2, a2.line(), a2.text(")")]));

        assert_eq!(render_pw_spaces(&a2, doc2, 8), "(\n  content\n)");
    }

    #[test]
    fn test_arena_if_break() {
        let a = DocArena::new();
        let doc = a.group(a.concat(&[
            a.text("("),
            a.if_break(a.text(",\n"), a.text(", ")),
            a.text(")"),
        ]));

        assert_eq!(render_pw_tab(&a, doc, 20), "(, )");
    }

    #[test]
    fn test_arena_dedent() {
        let a = DocArena::new();
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
    fn test_arena_multiline_text() {
        // Renders each `\n` as a hardline: first line in place, the rest broken.
        let a = DocArena::new();
        let doc = a.multiline_text("L0\nL1\nL2");
        assert_eq!(render_pw_tab(&a, doc, 100), "L0\nL1\nL2");

        // Output-identical to the per-line `concat([text, hardline, …])` it replaces.
        let a2 = DocArena::new();
        let concat = a2.concat(&[
            a2.text("L0"),
            a2.hardline(),
            a2.text("L1"),
            a2.hardline(),
            a2.text("L2"),
        ]);
        assert_eq!(render_pw_tab(&a2, concat, 100), "L0\nL1\nL2");
    }

    #[test]
    fn test_arena_multiline_text_context_indent() {
        // The node's reason for existing: the first line trails the preceding
        // content in place, every continuation line picks up the enclosing
        // indent level via its hardline.
        let a = DocArena::new();
        let doc = a.concat(&[a.text("parent"), a.indent(a.multiline_text("a\nb\nc"))]);
        assert_eq!(render_pw_tab(&a, doc, 80), "parenta\n\tb\n\tc");
    }

    #[test]
    fn test_arena_multiline_text_forces_break() {
        // Contains hardlines ⇒ `will_break`, so an enclosing group breaks without
        // a fits check — even at a width where the flat form would fit.
        let a = DocArena::new();
        let ml = a.multiline_text("a\nb");
        assert!(a.will_break(ml));
        let doc = a.group(a.concat(&[a.text("x"), a.line(), ml]));
        // Broke: the `line` is a newline ("x\na\nb"), not a space ("x a\nb").
        assert_eq!(render_pw_tab(&a, doc, 100), "x\na\nb");
    }

    #[test]
    fn test_arena_multiline_text_remove_lines() {
        // `remove_lines` must NOT touch a `MultilineText`: its `\n`s are hard lines, and
        // prettier's `removeLinesFn` gates on `!doc.hard` precisely so content that must
        // break still breaks.
        let a = DocArena::new();
        let flat = a.remove_lines(a.multiline_text("/*a\n b\n c*/"));
        assert_eq!(render_default(&a, flat), "/*a\n b\n c*/");
    }

    /// The glue this guards against, in the shape that shows it.
    ///
    /// The old behavior joined the lines with no separator, and the case above CANNOT see
    /// that: every line of `/*a\n b\n c*/` already starts with a space, so dropping the
    /// newlines still rendered `/*a b c*/` — which reads fine. Only a body whose lines
    /// would FUSE reveals it, so pin one.
    #[test]
    fn test_arena_multiline_text_remove_lines_does_not_glue_words() {
        let a = DocArena::new();
        let flat = a.remove_lines(a.multiline_text("/*text1\ntext2*/"));
        assert_eq!(
            render_default(&a, flat),
            "/*text1\ntext2*/",
            "flattening must not fuse `text1` and `text2` into `text1text2`"
        );
    }

    /// A hard line survives; a soft/normal one does not. The whole contract in one case.
    #[test]
    fn test_arena_remove_lines_keeps_hard_drops_soft_and_normal() {
        let a = DocArena::new();
        // Normal → space, soft → nothing: the flattening `remove_lines` really does do.
        let soft_and_normal = a.concat(&[
            a.text("a"),
            a.line(),
            a.text("b"),
            a.softline(),
            a.text("c"),
        ]);
        assert_eq!(render_default(&a, a.remove_lines(soft_and_normal)), "a bc");

        // Hard and literal → untouched, because removing one deletes a required newline.
        let hard = a.concat(&[a.text("a"), a.hardline(), a.text("b")]);
        assert_eq!(render_default(&a, a.remove_lines(hard)), "a\nb");
        let literal = a.concat(&[a.text("a"), a.literalline(), a.text("b")]);
        assert_eq!(render_default(&a, a.remove_lines(literal)), "a\nb");
    }

    #[test]
    fn test_arena_fill_all_fit() {
        let a = DocArena::new();
        let doc = a.fill(&[a.text("a"), a.line(), a.text("b"), a.line(), a.text("c")]);
        assert_eq!(render_pw_tab(&a, doc, 20), "a b c");
    }

    #[test]
    fn test_arena_fill_greedy_packing() {
        let a = DocArena::new();
        let doc = a.fill(&[a.text("aa"), a.line(), a.text("bb"), a.line(), a.text("cc")]);
        assert_eq!(render_pw_tab(&a, doc, 6), "aa bb\ncc");
    }

    #[test]
    fn test_arena_fill_long_comma_list() {
        let a = DocArena::new();
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
        let a = DocArena::new();
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
            indent: "\t",
            ..RenderConfig::default()
        };
        assert_eq!(
            render_test(&a, doc, &render, 0),
            "1, 2, 3, 4,\n\t5, 6, 7, 8"
        );

        let a2 = DocArena::new();
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
        let a = DocArena::new();
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let doc = a.join(docs, ", ");
        assert_eq!(render_default(&a, doc), "a, b, c");
    }

    #[test]
    fn test_arena_join_empty() {
        let a = DocArena::new();
        let docs: Vec<_> = vec![];
        let doc = a.join(docs, ", ");
        assert_eq!(render_default(&a, doc), "");
    }

    #[test]
    fn test_arena_join_doc_with_line() {
        let a = DocArena::new();
        let sep = a.line();
        let docs = vec![a.text("a"), a.text("b"), a.text("c")];
        let joined = a.join_doc(docs, sep);
        let doc = a.group(joined);

        assert_eq!(render_pw(&a, doc, 20), "a b c");

        let a2 = DocArena::new();
        let sep2 = a2.line();
        let docs2 = vec![a2.text("a"), a2.text("b"), a2.text("c")];
        let joined2 = a2.join_doc(docs2, sep2);
        let doc2 = a2.group(joined2);

        assert_eq!(render_pw(&a2, doc2, 3), "a\nb\nc");
    }

    #[test]
    fn test_arena_wrap() {
        let a = DocArena::new();
        let doc = a.wrap("(", a.text("content"), ")");
        assert_eq!(render_default(&a, doc), "(content)");
    }

    #[test]
    fn test_arena_parens() {
        let a = DocArena::new();
        let doc = a.parens(a.text("x"));
        assert_eq!(render_default(&a, doc), "(x)");
    }

    #[test]
    fn test_arena_brackets() {
        let a = DocArena::new();
        let doc = a.brackets(a.text("0"));
        assert_eq!(render_default(&a, doc), "[0]");
    }

    #[test]
    fn test_arena_braces() {
        let a = DocArena::new();
        let doc = a.braces(a.text("a: 1"));
        assert_eq!(render_default(&a, doc), "{a: 1}");
    }

    #[test]
    fn test_arena_indent_line() {
        let a = DocArena::new();
        let doc = a.group(a.concat(&[a.text("prefix"), a.indent_line(a.text("indented"))]));
        assert_eq!(render_pw_spaces(&a, doc, 10), "prefix\n  indented");
    }

    #[test]
    fn test_arena_indent_softline_flat() {
        let a = DocArena::new();
        let doc = a.group(a.concat(&[a.text("a"), a.indent_softline(a.text("b"))]));
        assert_eq!(render_pw_spaces(&a, doc, 20), "ab");
    }

    #[test]
    fn test_arena_fill_wraps_last_item_at_101() {
        let a = DocArena::new();
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
            indent: "\t",
            ..RenderConfig::default()
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
        let a = DocArena::new();
        let docs = vec![a.text("a")];
        let doc = a.join(docs, ", ");
        assert_eq!(render_default(&a, doc), "a");
    }

    #[test]
    fn test_arena_join_doc_with_comma_line() {
        let a = DocArena::new();
        let sep = a.concat(&[a.text(","), a.line()]);
        let docs = vec![a.text("item1"), a.text("item2"), a.text("item3")];
        let joined = a.join_doc(docs, sep);
        let doc = a.group(joined);

        assert_eq!(render_pw(&a, doc, 30), "item1, item2, item3");

        let a2 = DocArena::new();
        let sep2 = a2.concat(&[a2.text(","), a2.line()]);
        let docs2 = vec![a2.text("item1"), a2.text("item2"), a2.text("item3")];
        let joined2 = a2.join_doc(docs2, sep2);
        let doc2 = a2.group(joined2);

        assert_eq!(render_pw(&a2, doc2, 10), "item1,\nitem2,\nitem3");
    }

    // Regression guard for tsv's hardcoded `trailingComma: 'none'`: a bracketed
    // `join_doc` list gets inter-item commas but no trailing comma when it breaks.
    #[test]
    fn test_arena_join_doc_no_trailing_comma_in_brackets() {
        let a = DocArena::new();
        let sep = a.concat(&[a.text(","), a.line()]);
        let docs = vec![a.text("item1"), a.text("item2"), a.text("item3")];
        let joined = a.join_doc(docs, sep);
        let sl1 = a.softline();
        let inner = a.concat(&[sl1, joined]);
        let indented = a.indent(inner);
        let sl2 = a.softline();
        let doc = a.group(a.concat(&[a.text("["), indented, sl2, a.text("]")]));

        assert_eq!(render_pw_spaces(&a, doc, 30), "[item1, item2, item3]");

        let a2 = DocArena::new();
        let sep2 = a2.concat(&[a2.text(","), a2.line()]);
        let docs2 = vec![a2.text("item1"), a2.text("item2"), a2.text("item3")];
        let joined2 = a2.join_doc(docs2, sep2);
        let sl1_2 = a2.softline();
        let inner2 = a2.concat(&[sl1_2, joined2]);
        let indented2 = a2.indent(inner2);
        let sl2_2 = a2.softline();
        let doc2 = a2.group(a2.concat(&[a2.text("["), indented2, sl2_2, a2.text("]")]));

        // trailingComma: 'none' — no trailing comma when the list breaks
        assert_eq!(
            render_pw_spaces(&a2, doc2, 15),
            "[\n  item1,\n  item2,\n  item3\n]"
        );
    }

    #[test]
    fn test_arena_fill_single_item() {
        let a = DocArena::new();
        let doc = a.fill(&[a.text("hello")]);
        assert_eq!(render_default(&a, doc), "hello");
    }

    #[test]
    fn test_arena_fill_two_items() {
        let a = DocArena::new();
        let doc = a.fill(&[a.text("a"), a.line(), a.text("b")]);
        assert_eq!(render_pw_tab(&a, doc, 10), "a b");
    }

    #[test]
    fn test_arena_fill_none_fit() {
        let a = DocArena::new();
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
        let a = DocArena::new();
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
        let a = DocArena::new();
        let doc = a.group(a.concat(&[a.text("a"), a.indent_softline(a.text("b"))]));
        assert_eq!(render_pw_spaces(&a, doc, 1), "a\n  b");
    }

    #[test]
    fn test_arena_indent_softline_in_parens() {
        let a = DocArena::new();
        let sl = a.softline();
        let doc = a.group(a.concat(&[
            a.text("fn("),
            a.indent_softline(a.text("arg1, arg2")),
            sl,
            a.text(")"),
        ]));

        assert_eq!(render_pw_spaces(&a, doc, 30), "fn(arg1, arg2)");

        let a2 = DocArena::new();
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
        let a = DocArena::new();
        let doc = a.group(a.concat(&[a.text("a"), a.indent_line(a.text("b"))]));
        assert_eq!(render_pw_spaces(&a, doc, 20), "a b");
    }

    #[test]
    fn test_arena_wrap_with_nested_content() {
        let a = DocArena::new();
        let inner = a.concat(&[a.text("a"), a.text(", "), a.text("b")]);
        let doc = a.brackets(inner);
        assert_eq!(render_default(&a, doc), "[a, b]");
    }

    #[test]
    fn test_arena_nested_wraps() {
        let a = DocArena::new();
        let inner = a.brackets(a.text("x"));
        let doc = a.braces(a.concat(&[a.text(" "), inner, a.text(" ")]));
        assert_eq!(render_default(&a, doc), "{ [x] }");
    }

    #[test]
    fn test_arena_line_in_break_mode_doesnt_fit() {
        let a = DocArena::new();
        let doc = a.group(a.concat(&[a.text("hello"), a.line(), a.text("world")]));
        assert_eq!(render_pw_tab(&a, doc, 8), "hello\nworld");
    }

    #[test]
    fn test_conditional_group_picks_first_fitting_state() {
        // Three plain-text states of decreasing width; the renderer tries them in
        // order and renders the first whose flat form fits the print width.
        fn build(a: &DocArena) -> DocId {
            a.conditional_group(&[
                a.text("WWWWWWWWWW"), // width 10
                a.text("MMMMM"),      // width 5
                a.text("SS"),         // width 2
            ])
        }
        let a = DocArena::new();
        let doc = build(&a);
        assert_eq!(render_pw(&a, doc, 20), "WWWWWWWWWW"); // first state fits
        assert_eq!(render_pw(&a, doc, 7), "MMMMM"); // only the 5-wide state fits
        assert_eq!(render_pw(&a, doc, 3), "SS"); // only the 2-wide state fits
        // Nothing fits → fall back to the last state.
        assert_eq!(render_pw(&a, doc, 1), "SS");
    }

    #[test]
    fn test_conditional_group_single_state() {
        // A lone state (no expanded states) renders directly.
        let a = DocArena::new();
        let doc = a.conditional_group(&[a.text("x")]);
        assert_eq!(render_pw(&a, doc, 1), "x");
    }

    #[test]
    #[should_panic(expected = "conditional_group requires at least one state")]
    fn test_conditional_group_empty_panics() {
        let a = DocArena::new();
        a.conditional_group(&[]);
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

    // --- arena_fits flat-width fast-path guards ---
    //
    // `flat_width_memo` (arena_fits.rs) shortcuts break-free Flat subtrees with a
    // memoized width instead of walking them. The fast path and the slow walk are
    // two hand-maintained code paths that must stay byte-identical, so these tests
    // pin each per-variant arm: a future desync (a miscounted width, or a Some/None
    // that shortcuts a subtree that actually breaks) flips one of these assertions
    // rather than silently producing wrong layout.

    /// Fit `doc` in `width` columns in Flat mode, with no resolver (the docs here
    /// use only `Static`/`Pooled` text, never `Symbol`).
    fn fits_flat(a: &DocArena, doc: DocId, width: usize) -> bool {
        arena_fits(a, doc, width, Mode::Flat, None::<&dyn TextResolver>)
    }

    /// Assert the memoized flat width of `doc` is exactly `w`: it fits in `w` but
    /// not in `w - 1`. Any off-by-N in a fast-path arm flips one of these.
    fn assert_flat_width(a: &DocArena, doc: DocId, w: usize) {
        assert!(fits_flat(a, doc, w), "expected width {w} to fit");
        assert!(
            !fits_flat(a, doc, w - 1),
            "expected width {} not to fit",
            w - 1
        );
    }

    #[test]
    fn test_fits_flat_width_concat_and_lines() {
        let a = DocArena::new();
        // concat sums child widths
        assert_flat_width(&a, a.concat(&[a.text("abcd"), a.text("ef")]), 6);
        // Normal line = 1 (space) in flat; Soft line = 0
        assert_flat_width(&a, a.concat(&[a.text("ab"), a.line(), a.text("cd")]), 5);
        assert_flat_width(&a, a.concat(&[a.text("ab"), a.softline(), a.text("cd")]), 4);
        // fill is summed exactly like concat
        assert_flat_width(&a, a.fill(&[a.text("ab"), a.line(), a.text("cd")]), 5);
    }

    #[test]
    fn test_fits_flat_width_wrappers() {
        let a = DocArena::new();
        // a non-breaking group recurses into its contents
        let g = a.group(a.concat(&[a.text("ab"), a.line(), a.text("cd")]));
        assert_flat_width(&a, g, 5);
        // indent / dedent / align add no width in flat mode (they matter only at breaks)
        assert_flat_width(&a, a.indent(a.text("abc")), 3);
        assert_flat_width(&a, a.dedent(a.text("abcd")), 4);
        assert_flat_width(&a, a.align(4, a.text("ab")), 2);
        // line_suffix content is deferred, so it contributes 0 to the fit width
        let ls = a.concat(&[a.text("ab"), a.line_suffix(a.text("XXXXX")), a.text("cd")]);
        assert_flat_width(&a, ls, 4);
    }

    #[test]
    fn test_fits_flat_width_if_break_picks_flat_doc() {
        let a = DocArena::new();
        // In flat mode the flat_doc (", ", width 2) is measured, never break_doc (",\n").
        let doc = a.concat(&[
            a.text("("),
            a.if_break(a.text(",\n"), a.text(", ")),
            a.text(")"),
        ]);
        assert_flat_width(&a, doc, 4);
    }

    #[test]
    fn test_fits_flat_width_with_context_trailing_reserve() {
        let a = DocArena::new();
        let doc = a.with_context(
            a.text("abcd"),
            DocContext {
                trailing_reserve: 3,
                ..Default::default()
            },
        );
        // 4 content + 3 reserved = 7
        assert_flat_width(&a, doc, 7);
    }

    #[test]
    fn test_fits_flat_width_cached_non_ascii() {
        let a = DocArena::new();
        // "café" is non-ASCII, so its width is precomputed (cached_width = Some(4));
        // this exercises the cached-`Some(w)` arm rather than the resolve fallback.
        assert_flat_width(&a, a.text_pooled("café"), 4);
    }

    #[test]
    fn test_fits_flat_should_break_group_defers_to_walk() {
        let a = DocArena::new();
        // Flat content is 2+1+8 = 11 wide, but should_break forces Break mode in the
        // walk, where the inner line returns "fits" early. The fast path must NOT
        // shortcut this as an 11-wide flat subtree.
        let content = a.concat(&[a.text("ab"), a.line(), a.text("cdefghij")]);
        assert!(fits_flat(&a, a.group_break(content), 5));
        // contrast: a non-breaking group with identical content does not fit at 5
        let content2 = a.concat(&[a.text("ab"), a.line(), a.text("cdefghij")]);
        assert!(!fits_flat(&a, a.group(content2), 5));
    }

    #[test]
    fn test_fits_flat_hardline_defers_to_walk() {
        let a = DocArena::new();
        // hardline → the walk returns true after the leading text; a fast-path that
        // miscounted the hardline as 0 would compute width 4 and wrongly fail at 3.
        let doc = a.concat(&[a.text("ab"), a.hardline(), a.text("cd")]);
        assert!(fits_flat(&a, doc, 3));
    }

    #[test]
    fn test_fits_flat_break_parent_forces_false() {
        let a = DocArena::new();
        // BreakParent → the walk returns false even at unbounded width; a fast-path
        // that treated it as 0 width would wrongly report "fits".
        let doc = a.concat(&[a.text("ab"), a.break_parent(), a.text("cd")]);
        assert!(!fits_flat(&a, doc, 100));
    }

    #[test]
    fn test_fits_flat_newline_text_defers_to_walk() {
        let a = DocArena::new();
        // Static newline text: cached as HAS_NEWLINE (via the static width
        // cache), contains '\n' → walk returns true.
        assert!(fits_flat(&a, a.text("a\nb"), 0));
        // Pooled newline text: cached as HAS_NEWLINE → same early-true path.
        // Both cases pin the eager width policy (never NOT_COMPUTED), which is
        // what lets fits answer without borrowing the text pool.
        assert!(fits_flat(&a, a.text_pooled("café\nx"), 0));
        assert!(fits_flat(&a, a.text_pooled("a\nb"), 0));
    }
}
