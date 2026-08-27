// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Svelte's comment dedent reads line terminators with **two different classes**, and the
//! wire `value` is where the difference shows.
//!
//! `onComment` (`svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js`) dedents a
//! multi-line block comment by the indentation of the line it opens on, in two steps:
//!
//! - it finds that line with `while (a > 0 && source[a - 1] !== '\n') a -= 1` — `\n` and
//!   nothing else, so a `<CR>` / `<LS>` / `<PS>` ahead of the comment is ordinary text and
//!   the indentation taken is still the *line's* own;
//! - it strips that indentation with `value.replace(new RegExp('^' + indentation, 'gm'), '')`,
//!   and an `m`-mode `^` matches after every ECMAScript terminator — `\n`, `\r`, `<LS>`, `<PS>`.
//!
//! Reading either step with the other's class is a wire divergence in both directions: the
//! walk-back over the full class finds a line start Svelte has not got (so it dedents by
//! whatever `[ \t]` trails the terminator instead of by the real indent), and a `'\n'`-only
//! strip leaves the indent standing on lines Svelte does open. Both were live.
//!
//! **Why a test rather than a fixture.** Two of the five spellings cannot be a fixture
//! *input*: every parse-then-format entry point folds `<CR>` to `<LF>` before it parses
//! (`tsv_lang::printing::normalize_carriage_returns`), so a document carrying a raw `<CR>`
//! formats to its `<LF>` twin and is not the fixed point F1 requires. The `<LS>` / `<PS>`
//! spellings are deliberately not folded and *are* format-stable inside a verbatim region;
//! they are pinned by
//! `tests/fixtures/svelte/syntax/whitespace/line_terminators_comment_dedent`, whose
//! `expected.json` the oracle generates. What can live ONLY here is the `<CR>` half — the
//! other four spellings stay in the tables below too, since a shape that dropped them would
//! assert the class without ever running its null controls — and with them stays the
//! arrangement `<CR>` forces: every expectation is transcribed from the live modern Svelte parser
//! (`cargo run -p tsv_debug canonical_parse`) rather than regenerated, so it would go stale
//! silently if `onComment` changed.
//!
//! `tests/acorn_loc_line_terminators.rs` is the same arrangement for the *other* thing these
//! terminators decide — which line count a wire `loc` carries — and
//! `tests/comment_dedent_manufactured_source.rs` for the other thing the dedent itself reads
//! two ways: *which source* the indentation is measured out of.

/// The one comment's dedented wire `value` — the field `onComment` writes.
fn comment_value(src: &str) -> String {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept the component");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let comments = json["comments"]
        .as_array()
        .expect("wire root should carry a `comments` array");
    assert_eq!(comments.len(), 1, "each case carries exactly one comment");
    comments[0]["value"]
        .as_str()
        .expect("a comment's `value` should be a string")
        .to_owned()
}

/// Run one shape against every spelling of the class: `{T}` in `shape` is the terminator, and
/// each row pairs it with the wire `value` the oracle produces for that document.
fn assert_each_terminator(shape: &str, cases: [(&str, &str); 5]) {
    for (terminator, expected) in cases {
        let src = shape.replace("{T}", terminator);
        assert_eq!(comment_value(&src), expected, "terminator {terminator:?}");
    }
}

/// The comment's line is found by the `\n`-only walk-back, so a `<CR>` / `<LS>` / `<PS>`
/// between that line's start and the comment is not a line start: the indentation stripped is
/// the one the *line* opens with (`\t`, from `\tconst a = 1;`), not the empty run that
/// immediately follows the terminator.
///
/// `\n` and `<CR><LF>` are the null controls — a line really does start after them, and it
/// opens at column 0, so there is nothing to strip and the content rides out whole.
#[test]
fn walk_back_to_the_line_start_counts_newline_only() {
    assert_each_terminator(
        "<script lang=\"ts\">\n\tconst a = 1;{T}/* a1\n\ta2 */\n</script>\n",
        [
            ("\n", " a1\n\ta2 "),
            ("\r\n", " a1\n\ta2 "),
            ("\r", " a1\na2 "),
            ("\u{2028}", " a1\na2 "),
            ("\u{2029}", " a1\na2 "),
        ],
    );
}

/// The same walk-back with a *wider* indent behind the terminator than the line's own, so a
/// mis-read line start over-dedents here where the case above under-dedents. Both directions
/// matter: an indent that happens to match the line's own hides the bug outright, which is why
/// neither shape alone is the test.
#[test]
fn walk_back_ignores_a_wider_indent_behind_the_terminator() {
    assert_each_terminator(
        "<script lang=\"ts\">\n\tconst a = 1;{T}\t\t/* a1\n\t\ta2 */\n</script>\n",
        [
            ("\n", " a1\na2 "),
            ("\r\n", " a1\na2 "),
            ("\r", " a1\n\ta2 "),
            ("\u{2028}", " a1\n\ta2 "),
            ("\u{2029}", " a1\n\ta2 "),
        ],
    );
}

/// The other half takes the opposite class: `^` under the `m` flag opens a line at every
/// ECMAScript terminator, so the `\t` behind each one comes off — whichever of the five the
/// author wrote. Splitting the content on `'\n'` alone leaves it standing after four of them.
#[test]
fn indentation_is_stripped_after_every_terminator() {
    assert_each_terminator(
        "<script lang=\"ts\">\n\t/* c1\n\tc2{T}\tc3 */\n\tconst c = 1;\n</script>\n",
        [
            ("\n", " c1\nc2\nc3 "),
            ("\r\n", " c1\nc2\r\nc3 "),
            ("\r", " c1\nc2\rc3 "),
            ("\u{2028}", " c1\nc2\u{2028}c3 "),
            ("\u{2029}", " c1\nc2\u{2029}c3 "),
        ],
    );
}
