// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Every parse-then-format entry point folds `<CR>` and `<CR><LF>` to `<LF>` **before** it
//! parses, so a document formats to exactly what its `<LF>` twin formats to — byte for
//! byte, whatever its author's line endings were. `parse` deliberately does not fold: its
//! offsets are a drop-in contract with acorn / Svelte / `parseCss` over the author's own
//! bytes. See `tsv_lang::printing::normalize_carriage_returns`.
//!
//! The seams are covered here rather than only by fixtures because a fixture reaches
//! exactly one of them (the CLI's `format_source`, which every audit shares), while the
//! rule is what makes a *new* entry point correct. The bindings' three
//! (`tsv_ffi` / `tsv_napi` / `tsv_wasm`, one `parse_format!` each) are out of Rust's reach
//! and are covered by their own npm-package tests.
//!
//! The payload is deliberately an **indentable block comment sitting at the wrong indent**,
//! because that is the shape that fails when the fold happens too late. Answering `<CR>` on
//! the finished string instead leaves the doc-builder's line splits — `Comment::multiline`
//! at parse, `is_indentable_block_comment` at build —
//! reading a lone-`<CR>` document as ONE line: the comment rides out verbatim, the fold
//! then splits it, and the second pass re-indents what the first left alone. The `<CR><LF>`
//! twin is the null control on the same dimension: its `<CR>` lands at a line END, so it
//! survives a `'\n'` split and that spelling stayed correct throughout.

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

/// The LF form, its lone-`<CR>` twin, and its `<CR><LF>` twin must all format to the same
/// bytes — and that output must be a fixed point, since a formatter that agreed on pass one
/// and moved on pass two would be exactly the bug this guards.
fn assert_terminator_agnostic(lf: &str, parser: ParserType, label: &str) {
    let expected = format_ok(lf, parser, label);
    assert!(!expected.contains('\r'), "{label}: tsv's output is LF-only");
    assert_eq!(
        format_ok(&expected, parser, label),
        expected,
        "{label}: the formatted form must be a fixed point"
    );

    for (spelling, twin) in [
        ("cr", lf.replace('\n', "\r")),
        ("crlf", lf.replace('\n', "\r\n")),
    ] {
        assert_eq!(
            format_ok(&twin, parser, label),
            expected,
            "{label}: the {spelling} twin formats differently"
        );
    }
}

/// `format_source`, asserting rather than unwrapping so a parse failure names the case.
fn format_ok(source: &str, parser: ParserType, label: &str) -> String {
    let formatted = format_source(source, parser);
    assert!(formatted.is_ok(), "{label} must format: {formatted:?}");
    formatted.expect("asserted Ok above")
}

/// A `.ts` document whose JSDoc comment is indented three levels deeper than its printed
/// position, so the per-line re-indent has to fire.
const TS: &str = "function f() {\n\t\t\t/**\n\t\t\t * a\n\t\t\t */\n\treturn 1;\n}\n";

/// The same shape inside a `<script lang=\"ts\">`, plus a template literal and a
/// `<!-- -->` comment — the two regions whose bytes ride out verbatim.
const SVELTE: &str = concat!(
    "<script lang=\"ts\">\n",
    "\t\t\t/**\n",
    "\t\t\t * a\n",
    "\t\t\t */\n",
    "\tconst a = `text1\ntext2`;\n",
    "</script>\n",
    "\n",
    "<!-- c1\n\tc2 -->\n",
    "<div>text</div>\n",
);

/// A stylesheet whose block comment is likewise over-indented.
const CSS: &str = "div {\n\t\t\t/**\n\t\t\t * a\n\t\t\t */\n\tcolor: red;\n}\n";

#[test]
fn typescript_formats_the_same_however_its_lines_end() {
    assert_terminator_agnostic(TS, ParserType::TypeScript, "typescript");
}

#[test]
fn svelte_formats_the_same_however_its_lines_end() {
    assert_terminator_agnostic(SVELTE, ParserType::Svelte, "svelte");
}

#[test]
fn css_formats_the_same_however_its_lines_end() {
    assert_terminator_agnostic(CSS, ParserType::Css, "css");
}

/// The library's own fused entry points fold too — a caller that never touches the CLI
/// still gets LF-only output, in every language crate.
#[test]
fn the_fused_format_str_entry_points_fold_as_well() {
    assert_eq!(
        tsv_ts::format_str(&TS.replace('\n', "\r")).expect("ts"),
        tsv_ts::format_str(TS).expect("ts"),
    );
    assert_eq!(
        tsv_svelte::format_str(&SVELTE.replace('\n', "\r")).expect("svelte"),
        tsv_svelte::format_str(SVELTE).expect("svelte"),
    );
    assert_eq!(
        tsv_css::format_str(&CSS.replace('\n', "\r")).expect("css"),
        tsv_css::format_str(CSS).expect("css"),
    );
}

/// `parse` is the other side of the seam and must NOT fold: its byte offsets are a drop-in
/// contract over the author's own bytes, so a `<CR>` source keeps every span it had.
#[test]
fn parse_leaves_the_authors_line_terminators_alone() {
    let cr = "const a = 1;\rconst b = 2;\r";
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(cr, &arena).expect("parse");
    let json = tsv_ts::convert_ast_json(&program, cr);
    let second = json
        .pointer("/body/1")
        .and_then(|n| n.get("start"))
        .and_then(serde_json::Value::as_u64)
        .expect("second statement");
    // 13, not 12: the `<CR>` is one character and it is still there.
    assert_eq!(second, 13);
}
