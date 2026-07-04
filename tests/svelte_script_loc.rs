// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! `<script>` `Program.loc` tag-position override in the Svelte writer.
//!
//! Svelte's `read_script` overrides the script `Program`'s byte-space `loc` with
//! `locator(<script> tag start)` / `locator(</script> end)` — the line/column of
//! the tag's own `<`/`>`, not of the content span (`.../read/script.js`). So when
//! the `<script>` tag is *indented* (` <script>`, `\t<script>`, markup on the same
//! line), the `Program.loc.start.column` is the tag's column, not `0`. tsv fused
//! this override but hardcoded the start column to `0`, correct only when the tag
//! sits at column 0 (the overwhelmingly common case, which is why it hid).
//!
//! Not fixturable: prettier strips leading whitespace before `<script>` (` <script>`
//! → `<script>`), so `fixture_init` formats the trigger away — like the auto-close
//! and `:nth-*()` offsets, this lives as an offline root test with the columns
//! transcribed from the live modern Svelte parser (`tsv_debug canonical_parse`).

use serde_json::Value;

/// Parse `src`, convert to the wire AST, and return `<field>.content.loc` (field =
/// `"instance"` or `"module"`).
fn content_loc(src: &str, field: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept the script");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    json[field]["content"]["loc"].clone()
}

fn assert_loc(loc: &Value, sl: i64, sc: i64, el: i64, ec: i64) {
    assert_eq!(loc["start"]["line"].as_i64(), Some(sl), "start.line");
    assert_eq!(loc["start"]["column"].as_i64(), Some(sc), "start.column");
    assert_eq!(loc["end"]["line"].as_i64(), Some(el), "end.line");
    assert_eq!(loc["end"]["column"].as_i64(), Some(ec), "end.column");
}

/// One leading space → the `<script>` `<` sits at column 1, so `loc.start.column`
/// is 1 (not 0).
#[test]
fn leading_space_before_script() {
    let loc = content_loc(" <script>\n\tlet a = 1;\n</script>", "instance");
    assert_loc(&loc, 1, 1, 3, 9);
}

/// Three leading spaces → column 3.
#[test]
fn indented_three_spaces() {
    let loc = content_loc("   <script>\n\tlet a = 1;\n</script>", "instance");
    assert_loc(&loc, 1, 3, 3, 9);
}

/// Markup on the same line before the tag (`x <script>`) → column 2.
#[test]
fn preceding_text_on_tag_line() {
    let loc = content_loc("x <script>\n\tlet a = 1;\n</script>", "instance");
    assert_loc(&loc, 1, 2, 3, 9);
}

/// Regression guard: a tag at column 0 still reports column 0.
#[test]
fn tag_at_column_zero_unchanged() {
    let loc = content_loc("<script>\n\tlet a = 1;\n</script>", "instance");
    assert_loc(&loc, 1, 0, 3, 9);
}

/// A multibyte char before the tag: the column is a UTF-16 *char* column (`é` is
/// one char / two bytes), so `<script>` reports column 2, exercising the
/// byte→char `translate_column` path rather than the raw byte column.
#[test]
fn multibyte_before_tag_uses_char_column() {
    let loc = content_loc("é <script>\n\tlet a = 1;\n</script>", "instance");
    assert_loc(&loc, 1, 2, 3, 9);
}

/// The module script shares the same fused override path.
#[test]
fn module_script_indented() {
    let loc = content_loc(" <script module>\n\tlet a = 1;\n</script>", "module");
    assert_loc(&loc, 1, 1, 3, 9);
}
