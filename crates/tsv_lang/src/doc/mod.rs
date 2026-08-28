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
mod arena_render_suffix;
mod render_config;
#[cfg(feature = "swallow_check")]
pub mod swallow;
mod types;

// Types
pub use types::{CachedWidth, DocContext, DocText, GroupId, LineKind, Mode, PoolSpan};

/// Run `$copy` under a `match` on `$len` that names each short length in
/// `[$($k),*]` as its own arm.
///
/// **Every arm is the same expression, and that is the point.** The doc
/// builder's two hottest byte moves — `DocArena::alloc_children`'s child append
/// and the render loop's output write — carry payloads far smaller than the call
/// that was carrying them: a child range is exactly two `DocId`s in 65% of
/// calls, and the render write moves a *mean of 4.69 bytes* with 5% of calls
/// moving zero. `Vec::extend_from_slice` / `String::push_str` lower a
/// runtime-length `copy_nonoverlapping` to an indirect `memcpy@plt`, so those
/// calls cost more than the bytes. Inside an arm that has matched `len == k`,
/// LLVM knows the count is `k` and stores it inline instead — one `mov` for two
/// `DocId`s, nothing at all for the empty write.
///
/// ⚠️ **Do not "simplify" this to a bare `$copy`.** The arms are load-bearing
/// codegen, invisible to every test because the output is byte-identical either
/// way; collapsing them measured **+1.1–1.2% instructions** across five corpora.
/// Re-slicing to `&x[..k]` inside the arm is *worse*, not better — the arm
/// already establishes the length, and the re-slice only adds a bounds check
/// (measured +0.15% against this spelling, and +576 B).
///
/// ⚠️ **The arm list is a size decision, per call site.** Arms multiply by the
/// number of places the enclosing function is inlined into: the render write has
/// two call sites and affords nine arms for +2.4 KB, while `alloc_children` is
/// `#[inline]` at many sites and affords exactly **one** — a second arm there
/// crosses an inliner threshold and costs **+179 KB of `.text`**. Re-measure
/// `.text` before adding one.
///
/// ⚠️ **The constant is the lever, not the call count.** `write_indentation`
/// spells this mechanism by hand, and its ladder priced the difference: turning
/// its per-level one-byte pushes into a single run-slice call — the change that
/// removes the most *calls* — is worth ~0.03% of a run, while naming the short
/// depths as constant-length arms on top of it is worth ~0.2%. A runtime-length
/// copy costs about the same whether it moves one byte or four, so collapsing a
/// per-byte loop buys little; the win is in the length becoming a constant. That
/// site keeps its own `match` rather than calling this macro because its arms
/// each need a *different* value (a tab run of the matched length), and the
/// re-slice spelling this macro warns against stays worse even when the source
/// string and the index are both compile-time constants.
macro_rules! specialize_short_len {
	($len:expr, [$($k:literal),* $(,)?], $copy:expr) => {
		match $len {
			$($k => $copy,)*
			_ => $copy,
		}
	};
}

pub(crate) use specialize_short_len;

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
    arena_measure_doc_flat_resolved, arena_print_doc, arena_print_doc_with_indent_resolved_into,
    arena_print_doc_with_indent_resolved_preserve_whitespace_into,
};

// Arena fits
pub use arena_fits::arena_fits;

#[cfg(test)]
mod arena_tests {
    use super::arena::{DocArena, DocId};
    use super::arena_render::arena_print_doc_with_indent_and_render;
    use super::render_config::RenderConfig;
    #[cfg(feature = "comment_check")]
    use super::render_config::RenderPurpose;
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
            #[cfg(feature = "comment_check")]
            purpose: RenderPurpose::Output,
        };
        render_test(arena, doc, &render, 0)
    }

    /// Test helper: render with explicit `print_width` and tab indent.
    fn render_pw_tab(arena: &DocArena, doc: DocId, print_width: usize) -> String {
        let render = RenderConfig {
            print_width,
            indent: "\t",
            #[cfg(feature = "comment_check")]
            purpose: RenderPurpose::Output,
        };
        render_test(arena, doc, &render, 0)
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

    /// The four arms of `concat` — empty, single, pair, and the general slice —
    /// render identically to their contents, and the two degenerate ones return
    /// an existing `DocId` rather than allocating a `Concat` node. The arms are
    /// separate functions (an inlined dispatch over out-of-line allocating
    /// arms), so the boundaries between them are worth pinning directly.
    #[test]
    fn test_arena_concat_arms() {
        let a = DocArena::new();
        let t = a.text("x");

        assert_eq!(a.concat(&[]), a.empty());
        assert_eq!(a.concat(&[t]), t);
        assert_eq!(render_default(&a, a.concat(&[])), "");
        assert_eq!(render_default(&a, a.concat(&[t])), "x");
        assert_eq!(
            render_default(&a, a.concat(&[a.text("a"), a.text("b")])),
            "ab"
        );
        assert_eq!(
            render_default(&a, a.concat(&[a.text("a"), a.text("b"), a.text("c")])),
            "abc"
        );

        // A pair and a 3+ slice both allocate, and each child range resolves to
        // its own children — the pair arm builds its slice itself, so a shared
        // or mis-sized range would surface here and nowhere else in this crate.
        let pair = a.concat(&[a.text("1"), a.text("2")]);
        let triple = a.concat(&[a.text("3"), a.text("4"), a.text("5")]);
        assert_ne!(pair, triple);
        assert_eq!(render_default(&a, pair), "12");
        assert_eq!(render_default(&a, triple), "345");
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

    /// Atomizing a `conditional_group` must yield its *least*-expanded state — what
    /// prettier's re-render at `printWidth: Infinity` would pick.
    ///
    /// The states are dead once every line is flattened. Keeping them let render fall
    /// through to the most-expanded one at the real width and emit its separators as
    /// literal spaces (`fn( a, b )`) — the template-interpolation bug this guards.
    #[test]
    fn test_arena_atomize_collapses_conditional_group() {
        let a = DocArena::new();
        let flat = a.concat(&[
            a.text("fn("),
            a.text("a"),
            a.text(", "),
            a.text("b"),
            a.text(")"),
        ]);
        let expanded = a.concat(&[
            a.text("fn("),
            a.indent(a.concat(&[a.line(), a.text("a"), a.text(","), a.line(), a.text("b")])),
            a.line(),
            a.text(")"),
        ]);
        let cg = a.conditional_group(&[flat, expanded]);

        // Far too narrow for any state to fit — without the collapse, render picks the
        // most-expanded state and its flattened `line`s surface as spaces.
        assert_eq!(render_pw_tab(&a, a.atomize(cg), 5), "fn(a, b)");
    }

    /// **The contract of `atomize`, asserted directly: the result renders
    /// identically at every width.**
    ///
    /// It emulates prettier's re-render at `printWidth: Infinity`, so width must stop
    /// being an input. Any node where flattening disagrees with "what would infinite width
    /// print?" shows up here as a width-dependent string — which is precisely how the
    /// `conditional_group` bug behaved (`fn(a, b)` wide, `fn( a, b )` narrow) while every
    /// external gate stayed green: the output was still idempotent, still reparsed, and
    /// still dropped no comment, so only a prettier diff could see it.
    ///
    /// Prefer extending this over adding another single-width case — a new doc shape gets
    /// graded at every width for free.
    #[test]
    fn test_arena_atomize_is_width_invariant() {
        let a = DocArena::new();
        let inner_cg = a.conditional_group(&[
            a.concat(&[a.text("g("), a.text("x"), a.text(")")]),
            a.concat(&[
                a.text("g("),
                a.indent(a.concat(&[a.line(), a.text("x")])),
                a.line(),
                a.text(")"),
            ]),
        ]);
        let shapes: &[(&str, DocId)] = &[
            ("conditional_group", {
                let flat = a.concat(&[
                    a.text("fn("),
                    a.text("a"),
                    a.text(", "),
                    a.text("b"),
                    a.text(")"),
                ]);
                let expanded = a.concat(&[
                    a.text("fn("),
                    a.indent(a.concat(&[
                        a.line(),
                        a.text("a"),
                        a.text(","),
                        a.line(),
                        a.text("b"),
                    ])),
                    a.line(),
                    a.text(")"),
                ]);
                a.conditional_group(&[flat, expanded])
            }),
            (
                "nested conditional_group",
                a.concat(&[a.text("outer("), inner_cg, a.text(")")]),
            ),
            (
                "plain group",
                a.group(a.concat(&[a.text("a"), a.line(), a.text("b")])),
            ),
            (
                "group_break",
                a.group_break(a.concat(&[a.text("a"), a.line(), a.text("b")])),
            ),
            (
                "if_break",
                a.group(a.concat(&[a.text("a"), a.if_break(a.text("B"), a.text("F"))])),
            ),
            (
                "fill",
                a.fill(&[a.text("a"), a.line(), a.text("b"), a.line(), a.text("c")]),
            ),
            (
                "hardline",
                a.concat(&[a.text("a"), a.hardline(), a.text("b")]),
            ),
            (
                "indent + softline",
                a.indent(a.concat(&[a.softline(), a.text("a"), a.softline(), a.text("b")])),
            ),
        ];

        for &(label, doc) in shapes {
            let flat = a.atomize(doc);
            let wide = render_pw_tab(&a, flat, 10_000);
            for width in [1, 2, 5, 13, 40, 100] {
                assert_eq!(
                    render_pw_tab(&a, flat, width),
                    wide,
                    "{label}: atomized doc rendered differently at width {width} than at infinite width"
                );
            }
            assert!(
                !wide.contains('\n'),
                "{label}: atomized doc kept a newline: {wide:?}"
            );
        }
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
            #[cfg(feature = "comment_check")]
            purpose: RenderPurpose::Output,
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
            #[cfg(feature = "comment_check")]
            purpose: RenderPurpose::Output,
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

    // ---------------------------------------------------------------------
    // Fill render policies — the per-fill layout flags on `DocContext`.
    //
    // Each is set by exactly one builder in `tsv_svelte`, and each was reachable only
    // through that crate's fixture suite: the crate that OWNS the render behavior had no
    // test for it, so a change here could only be graded a crate away. Every test below
    // renders the same fill parts twice, with the flag and without, so the assertion is
    // about the flag rather than about fills in general — a control that also documents
    // what the default fill does at the same site.
    // ---------------------------------------------------------------------

    /// `after_element_fold`, head hug: a breakable head that does not fit on its own line
    /// *either* renders in place and breaks internally, instead of dropping to a fresh line
    /// — which would only strand a break in front of it (the `>⏎<child` non-idempotency).
    #[test]
    fn test_fill_after_element_fold_hugs_wide_head() {
        let a = DocArena::new();
        // Flat width 16, wider than the print width even at column 0.
        let head = a.group(a.concat(&[a.text("AAAAAAAA"), a.softline(), a.text("BBBBBBBB")]));
        let parts = [head, a.line(), a.text("tail")];
        let fold = a.with_context(
            a.fill(&parts),
            DocContext::default().with_after_element_fold(true),
        );
        let plain = a.fill(&parts);

        // `>` is the small prefix that puts the fill mid-line (the parent element's `>`).
        assert_eq!(
            render_pw_tab(&a, a.concat(&[a.text(">"), fold]), 10),
            ">AAAAAAAA\nBBBBBBBB\ntail"
        );
        assert_eq!(
            render_pw_tab(&a, a.concat(&[a.text(">"), plain]), 10),
            ">\nAAAAAAAA\nBBBBBBBB\ntail"
        );
    }

    /// `after_element_fold`, terminal-tail hug: once the head has wrapped at line start, the
    /// trailing item hugs the dangled last line when it fits at the resulting column. Every
    /// other fill isolates a wrapped item from what follows it.
    #[test]
    fn test_fill_after_element_fold_tail_hugs_wrapped_head() {
        let a = DocArena::new();
        // Flat width 12: wraps at print width 10, ending at column 6.
        let head = a.group(a.concat(&[a.text("AAAAAA"), a.softline(), a.text("BBBBBB")]));
        let parts = [head, a.line(), a.text("t")];
        let fold = a.with_context(
            a.fill(&parts),
            DocContext::default().with_after_element_fold(true),
        );

        assert_eq!(render_pw_tab(&a, fold, 10), "AAAAAA\nBBBBBB t");
        assert_eq!(render_pw_tab(&a, a.fill(&parts), 10), "AAAAAA\nBBBBBB\nt");
    }

    /// `break_before_wide_flow`: the fill's trailing separator measures the *following*
    /// render-stack node as a whole FLAT unit, so a node that cannot fit flat after the
    /// separator drops to its own line intact. Without it the following node is measured in
    /// its inherited Break mode, where `arena_fits` short-circuits at the node's first
    /// internal line and wrongly reports a fit — packing it onto this line and breaking it
    /// in place instead.
    #[test]
    fn test_fill_break_before_wide_flow_measures_next_flat() {
        let a = DocArena::new();
        let parts = [a.text("word"), a.line()];
        let wide = a.group(a.concat(&[a.text("<AAAA"), a.softline(), a.text("BBBB>")]));
        let flow = a.with_context(
            a.fill(&parts),
            DocContext::default().with_break_before_wide_flow(true),
        );

        // 4 + 1 + 10 = 15 > 12, so the separator breaks and `wide` renders intact at column 0.
        assert_eq!(
            render_pw_tab(&a, a.concat(&[flow, wide]), 12),
            "word\n<AAAABBBB>"
        );
        // Control: the Break-mode short-circuit reports a fit, `wide` packs onto the word's
        // line, and it is the node's own content that breaks.
        assert_eq!(
            render_pw_tab(&a, a.concat(&[a.fill(&parts), wide]), 12),
            "word <AAAA\nBBBB>"
        );
    }

    /// The fill's last item is measured WITH the rest of the render stack (the default
    /// look-ahead): a node byte-glued to the last word belongs to its fit check, so the
    /// fill breaks in front of the word and the welded pair travels to the fresh line
    /// together rather than the tail riding past print width. The smallest welded unit —
    /// one word plus its glued tag — takes the same travel rule as any welded run.
    #[test]
    fn test_fill_last_item_measures_glued_rest() {
        let a = DocArena::new();
        let parts = [a.text("word")];
        let pad = a.text("PADDING");
        // Stands in for the Svelte `{tag}` this rule exists for — braces would read as
        // format arguments to clippy.
        let tag = a.text("[tag]");

        // `word[tag]` (9) does not fit the 5 columns after `PADDING`: the fill breaks in
        // front of `word` and the pair travels together.
        assert_eq!(
            render_pw_tab(&a, a.concat(&[pad, a.fill(&parts), tag]), 12),
            "PADDING\nword[tag]"
        );
        // At 16 the pair fits after the padding and packs.
        assert_eq!(
            render_pw_tab(&a, a.concat(&[pad, a.fill(&parts), tag]), 16),
            "PADDINGword[tag]"
        );
    }

    /// `glued_lead`: the fill's FIRST item is byte-glued to what precedes it, so the boundary
    /// in front of it carries no whitespace and the fresh-line drop would inject a rendered
    /// space. The head renders in place and the run breaks at its first *internal* whitespace
    /// boundary instead. Head only — every later item keeps the ordinary drop.
    #[test]
    fn test_fill_glued_lead_keeps_head_in_place() {
        let a = DocArena::new();
        let parts = [
            a.text("wwwwww"),
            a.line(),
            a.text("xxxx"),
            a.line(),
            a.text("y"),
        ];
        let glued = a.with_context(a.fill(&parts), DocContext::default().with_glued_lead(true));
        let pad = a.text("PADDING");

        // The head does not fit in the 5 remaining columns but does fit at column 0, so the
        // control drops it; the glued fill must not, since there is no break point there.
        assert_eq!(
            render_pw_tab(&a, a.concat(&[pad, glued]), 12),
            "PADDINGwwwwww\nxxxx y"
        );
        assert_eq!(
            render_pw_tab(&a, a.concat(&[pad, a.fill(&parts)]), 12),
            "PADDING\nwwwwww\nxxxx y"
        );
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
    fn test_gated_state_admission_tracks_the_probe() {
        // A miniature member-chain hug window (see `DocNode::GatedState`):
        //   state 0 — everything flat:        `x.f().m({ aaaa })`   (17 wide)
        //   state 1 — gated hug:              `x.f().m({⏎  aaaa⏎})` (head 9 wide)
        //   state 2 — expanded fallback:      `x⏎  .f()⏎  m({ aaaa })`
        // The probe is the last group's flat form, `m({ aaaa })` (11 wide),
        // measured on a fresh line one indent level deeper (tab = 2 → 13
        // columns needed). The hug is admitted only while that line cannot
        // hold it — otherwise the fallback keeping the argument flat is the
        // settled form and must win.
        fn build(a: &DocArena) -> DocId {
            let last_flat = a.concat(&[a.text("m("), a.text("{ aaaa }"), a.text(")")]);
            let one_line = a.concat(&[a.text("x.f()."), last_flat]);
            let hug = a.concat(&[
                a.text("x.f()."),
                a.group_break(a.concat(&[
                    a.text("m({"),
                    a.indent(a.concat(&[a.hardline(), a.text("aaaa")])),
                    a.hardline(),
                    a.text("})"),
                ])),
            ]);
            let expanded = a.concat(&[
                a.text("x"),
                a.indent(a.concat(&[a.hardline(), a.text(".f()"), a.hardline(), last_flat])),
            ]);
            a.conditional_group(&[one_line, a.gated_state(last_flat, hug), expanded])
        }
        let a = DocArena::new();
        let doc = build(&a);
        // Everything fits flat → state 0.
        assert_eq!(render_pw_tab(&a, doc, 20), "x.f().m({ aaaa })");
        // Flat overflows, but the probe fits on the fallback's continuation
        // line (2 + 11 = 13 ≤ 14) → the hug is SKIPPED → expanded fallback.
        assert_eq!(render_pw_tab(&a, doc, 14), "x\n\t.f()\n\tm({ aaaa })");
        // Probe cannot fit (13 > 12) and the hug's head fits (9 ≤ 12) → hug.
        assert_eq!(render_pw_tab(&a, doc, 12), "x.f().m({\n\taaaa\n})");
        // Probe cannot fit AND the hug's head overflows (9 > 8) → fallback.
        assert_eq!(render_pw_tab(&a, doc, 8), "x\n\t.f()\n\tm({ aaaa })");
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

    /// Fit `doc` in `width` columns in Flat mode, with no source (the docs here
    /// use only `Static`/`Pooled` text, never `SourceSpan`).
    fn fits_flat(a: &DocArena, doc: DocId, width: usize) -> bool {
        arena_fits(a, doc, width, Mode::Flat, None)
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

    // --- lineSuffixBoundary: the flush ends the line, and doesn't fit ---
    //
    // The two halves of one rule (Prettier's pushed `hardlineWithoutBreakParent` +
    // its `fits` `hasLineSuffix`). Both are invisible to every output-diff gate
    // when broken: the swallowed form is still *a* string, and the mismeasured one
    // is still valid code — only the column moves. See `docs/comments.md`.

    #[test]
    fn test_line_suffix_boundary_flush_ends_the_line() {
        let a = DocArena::new();
        // `a` + a deferred `// c` + a boundary + `b`. Flushing the comment inline
        // would put `b` INSIDE it — the swallow this node exists to prevent.
        let doc = a.concat(&[
            a.text("a"),
            a.line_suffix(a.text(" // c")),
            a.line_suffix_boundary(),
            a.text("b"),
        ]);
        assert_eq!(render_default(&a, doc), "a // c\nb");
    }

    #[test]
    fn test_line_suffix_boundary_without_pending_suffix_is_inert() {
        let a = DocArena::new();
        // No suffix queued → no flush, hence no break: the boundary is a no-op.
        let doc = a.concat(&[a.text("a"), a.line_suffix_boundary(), a.text("b")]);
        assert_eq!(render_default(&a, doc), "ab");
    }

    #[test]
    fn test_line_suffix_boundary_group_breaks_rather_than_measuring_across_it() {
        let a = DocArena::new();
        // The assignment shape (`fluid_after_operator`): the marker group is
        // measured with the boundary in its look-ahead. It fits on width alone, so
        // only the pending suffix can break it — and it must, since the flush ends
        // the line the group would otherwise have measured flat.
        let group = a.group(a.indent(a.line()));
        let doc = a.concat(&[
            a.text("x ="),
            a.line_suffix(a.text(" // c")),
            group,
            a.line_suffix_boundary(),
            a.text("y"),
        ]);
        assert_eq!(render_pw(&a, doc, 100), "x = // c\n\ty");
    }

    #[test]
    fn test_fits_stops_at_a_boundary_with_a_suffix_pending() {
        let a = DocArena::new();
        // Everything here is 3 columns wide flat, so width alone never decides.
        let no_suffix = a.concat(&[a.text("a"), a.line_suffix_boundary(), a.text("bc")]);
        assert!(fits_flat(&a, no_suffix, 3));
        // With a suffix queued before it, the boundary ends the line: what follows
        // is not on this line, so the fit is decided at the boundary and fails.
        let with_suffix = a.concat(&[
            a.text("a"),
            a.line_suffix(a.text("XXXXX")),
            a.line_suffix_boundary(),
            a.text("bc"),
        ]);
        assert!(!fits_flat(&a, with_suffix, 3));
        // …and the order matters: a suffix queued AFTER the boundary is irrelevant.
        let suffix_after = a.concat(&[
            a.text("a"),
            a.line_suffix_boundary(),
            a.line_suffix(a.text("XXXXX")),
            a.text("bc"),
        ]);
        assert!(fits_flat(&a, suffix_after, 3));
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
        let doc = a.with_context(a.text("abcd"), DocContext::reserving(3));
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

    // --- flush_break: force only the group the deferred run flushes in ---
    //
    // The scoped alternative to BreakParent for a deferred trailing run whose
    // construct is STRIPPED from the output: the group owning the next line
    // opportunity after the node must break (the flush lands there), while a
    // group with no line after it stays flat — the unscoped force there was a
    // break the reparse could not reproduce (format∘format ≠ format). See the
    // stripped paren shell in `tsv_ts` (`build_parenthesized_type_unwrap_doc`).

    #[test]
    fn test_flush_break_breaks_the_flush_group_not_the_closed_one() {
        let a = DocArena::new();
        // The union/intersection shape: the suffix + flush sit inside `inner`
        // (the intersection), whose only line is BEFORE them; the next line
        // opportunity is `outer`'s if_break separator. Outer must break — the
        // deferred comment flushes at its separator line — while inner, with
        // nothing left to put on a new line, stays flat.
        let inner = a.group(a.concat(&[
            a.text("(B"),
            a.line(),
            a.text("& A"),
            a.line_suffix(a.text(" // c")),
            a.flush_break(),
            a.text(")"),
        ]));
        let sep = a.if_break(a.concat(&[a.line(), a.text("| ")]), a.text(" | "));
        let outer = a.group(a.concat(&[inner, sep, a.text("C")]));
        assert_eq!(render_pw(&a, outer, 100), "(B & A) // c\n| C");
    }

    #[test]
    fn test_fits_flat_flush_break_without_following_line_fits() {
        let a = DocArena::new();
        // No line opportunity after the node → nothing this group could break
        // to flush the run → it fits (contrast BreakParent's unconditional
        // false above). This is the intermediate-group half of the contract.
        let doc = a.concat(&[
            a.text("ab"),
            a.line_suffix(a.text("X")),
            a.flush_break(),
            a.text("cd"),
        ]);
        assert!(fits_flat(&a, doc, 100));
    }

    #[test]
    fn test_fits_flat_flush_break_vetoes_a_following_flat_line() {
        let a = DocArena::new();
        // A breakable line after the node is the flush's landing — measured
        // flat it renders no line end, so the group must break: doesn't fit.
        let doc = a.concat(&[
            a.text("ab"),
            a.line_suffix(a.text("X")),
            a.flush_break(),
            a.line(),
            a.text("cd"),
        ]);
        assert!(!fits_flat(&a, doc, 100));
        // …and order matters: a line BEFORE the node is not the flush point.
        let line_before = a.concat(&[
            a.text("ab"),
            a.line(),
            a.line_suffix(a.text("X")),
            a.flush_break(),
            a.text("cd"),
        ]);
        assert!(fits_flat(&a, line_before, 100));
    }

    #[test]
    fn test_fits_flat_flush_break_vetoes_an_if_break_with_a_breakable_arm() {
        let a = DocArena::new();
        // The composite separator shape: flat the if_break renders " | " (no
        // line end), but its break arm holds one — the group must break to
        // take it, so measured flat it doesn't fit…
        let sep = a.if_break(a.concat(&[a.line(), a.text("| ")]), a.text(" | "));
        let doc = a.concat(&[
            a.text("ab"),
            a.line_suffix(a.text("X")),
            a.flush_break(),
            sep,
            a.text("cd"),
        ]);
        assert!(!fits_flat(&a, doc, 100));
        // …while an if_break whose break arm has no line to offer changes
        // nothing and the walk continues into the flat arm.
        let lineless = a.if_break(a.text(","), a.empty());
        let doc2 = a.concat(&[
            a.text("ab"),
            a.line_suffix(a.text("X")),
            a.flush_break(),
            lineless,
            a.text("cd"),
        ]);
        assert!(fits_flat(&a, doc2, 100));
    }

    #[test]
    fn test_flush_break_is_invisible_to_will_break_and_render() {
        let a = DocArena::new();
        // No particular group is forced by the subtree query (the fits walk
        // decides per group), and the node renders nothing.
        let doc = a.concat(&[a.text("a"), a.flush_break(), a.text("b")]);
        assert!(!a.will_break(doc));
        assert_eq!(render_default(&a, doc), "ab");
    }

    #[test]
    fn test_flush_break_pending_meets_a_hard_line_in_the_lookahead() {
        let a = DocArena::new();
        // The pending state rides into the rest-commands look-ahead like
        // `has_line_suffix` does — and a HARD line there already ends the line
        // (the flush lands on it), so the measured group has nothing left to
        // break for and stays flat. Only a *breakable* line while pending vetoes.
        let inner = a.group(a.concat(&[
            a.text("(B"),
            a.line(),
            a.text("& A"),
            a.line_suffix(a.text(" // c")),
            a.flush_break(),
            a.text(")"),
        ]));
        let doc = a.concat(&[inner, a.hardline(), a.text("C")]);
        assert_eq!(render_pw(&a, doc, 100), "(B & A) // c\nC");
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
