// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A Svelte or CSS document whose formatted output **begins with a U+FEFF that is content**
//! is written with a byte-order mark ahead of it. A UTF-8 decode strips exactly one leading
//! BOM, so without one the next read takes the content character for a BOM and drops it —
//! an F1 break and, in Svelte, a render change (U+FEFF is not HTML whitespace, so the text
//! node renders). `BOM + U+FEFF + …` is the only lossless spelling of such a text, and it is
//! its own fixed point: the read strips the BOM, the content U+FEFF is output byte 0 again,
//! and the BOM is written again. A BOM with nothing load-bearing behind it is still
//! stripped (`bom_prettier_divergence`).
//!
//! TypeScript is the null control: ecma262 §12.2 makes `<ZWNBSP>` `WhiteSpace`, so a leading
//! U+FEFF is trivia there and the formatter drops it, with no BOM to write.
//!
//! The fixtures (`svelte/syntax/whitespace/leading_zwnbsp_prettier_divergence` and its CSS
//! sibling) reach one entry point, the CLI's `format_source`. The rule has to hold at every
//! one, so this asserts the same bytes from each: `format_source`, each crate's
//! `format_str`, and the unfolded `format` / `format_in` over a caller's own parse. The
//! bindings' `parse_format!` exports reach the crates' `format_folded_in`, as
//! `format_source` does.

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

const BOM: &str = "\u{FEFF}";

/// Format `source` through every Rust entry point for `parser`, assert they agree, and
/// return the shared output.
fn format_all(source: &str, parser: ParserType, label: &str) -> String {
    let via_cli = format_source(source, parser);
    assert!(
        via_cli.is_ok(),
        "{label}: format_source failed: {via_cli:?}"
    );
    let via_cli = via_cli.expect("asserted Ok above");

    let arena = bumpalo::Bump::new();
    let doc_arena = tsv_lang::doc::arena::DocArena::for_source(source);
    let (via_str, via_format, via_format_in) = match parser {
        ParserType::Svelte => {
            let ast = tsv_svelte::parse(source, &arena).expect("svelte parse");
            (
                tsv_svelte::format_str(source).expect("svelte format_str"),
                tsv_svelte::format(&ast, source),
                tsv_svelte::format_in(&ast, source, &doc_arena),
            )
        }
        ParserType::Css => {
            let ast = tsv_css::parse(source, &arena).expect("css parse");
            (
                tsv_css::format_str(source).expect("css format_str"),
                tsv_css::format(&ast, source),
                tsv_css::format_in(&ast, source, &doc_arena),
            )
        }
        ParserType::TypeScript => {
            let ast = tsv_ts::parse(source, &arena).expect("ts parse");
            let via_format = tsv_ts::format(&ast, source);
            (
                tsv_ts::format_str(source).expect("ts format_str"),
                via_format.clone(),
                via_format,
            )
        }
    };
    assert_eq!(
        via_str, via_cli,
        "{label}: format_str disagrees with format_source"
    );
    assert_eq!(
        via_format, via_cli,
        "{label}: format disagrees with format_source"
    );
    assert_eq!(
        via_format_in, via_cli,
        "{label}: format_in disagrees with format_source"
    );
    via_cli
}

/// `source` formats to exactly `expected` at every entry point, and `expected` is a fixed
/// point.
fn assert_formats_to(source: &str, parser: ParserType, expected: &str, label: &str) {
    let out = format_all(source, parser, label);
    assert_eq!(out, expected, "{label}: pass 1");
    assert_eq!(
        format_all(&out, parser, label),
        out,
        "{label}: the formatted form must be a fixed point"
    );
}

// --- Svelte: a leading U+FEFF of template text ---

/// A U+FEFF of template text behind a space: the space is trimmed and the U+FEFF becomes
/// output byte 0.
#[test]
fn svelte_text_behind_a_space_gets_a_bom() {
    assert_formats_to(
        " \u{FEFF}<div></div>",
        ParserType::Svelte,
        "\u{FEFF}\u{FEFF}\n<div></div>\n",
        "svelte text before a block element",
    );
}

/// A real BOM ahead of the content U+FEFF round-trips: the BOM is stripped at the read and
/// written back because the content still leads.
#[test]
fn svelte_real_bom_then_text_zwnbsp_round_trips() {
    assert_formats_to(
        "\u{FEFF}\u{FEFF}\n<div></div>\n",
        ParserType::Svelte,
        "\u{FEFF}\u{FEFF}\n<div></div>\n",
        "svelte BOM + text U+FEFF",
    );
}

/// The same rule wherever the leading text lands: before an inline tag, a comment, prose.
#[test]
fn svelte_text_zwnbsp_before_other_first_nodes() {
    for (source, expected) in [
        (" \u{FEFF}{x}", "\u{FEFF}\u{FEFF}{x}\n"),
        (" \u{FEFF}<!-- c -->", "\u{FEFF}\u{FEFF}<!-- c -->\n"),
        (" \u{FEFF}text", "\u{FEFF}\u{FEFF}text\n"),
    ] {
        assert_formats_to(source, ParserType::Svelte, expected, source);
    }
}

/// Where the output does NOT begin with the content U+FEFF, no BOM is written: a U+FEFF
/// inside a `<style>` / `<script>` block sits behind its tag.
#[test]
fn svelte_zwnbsp_not_at_output_start_gets_no_bom() {
    for source in [
        "<style>\u{FEFF}a{}</style>",
        "<script>\u{FEFF}let a;</script>",
    ] {
        let out = format_all(source, ParserType::Svelte, source);
        assert!(
            !out.starts_with(BOM),
            "{source:?}: no BOM expected, got {out:?}"
        );
        assert_eq!(
            format_all(&out, ParserType::Svelte, source),
            out,
            "{source:?}: F1"
        );
    }
}

/// A document that is ONLY the U+FEFF has no content to keep: the text is also the
/// document's trailing edge, which Svelte trims as whitespace (JavaScript `\s` holds
/// U+FEFF), so it renders nothing and the formatter prints nothing. No BOM is written for an
/// empty output.
#[test]
fn svelte_document_that_is_only_a_zwnbsp_formats_empty() {
    for source in [
        " \u{FEFF}",
        " \u{FEFF}\n",
        "\u{FEFF}\u{FEFF}",
        "\u{FEFF}\u{FEFF}\n",
    ] {
        assert_formats_to(source, ParserType::Svelte, "", source);
    }
}

/// A `<style>` nested inside an element is formatted as a stylesheet FRAGMENT, not a
/// document: its content's first character is not a file's first byte, so a U+FEFF there is
/// content — never a BOM to strip, and never one to write. It formats the way the top-level
/// `<style>` island does, one indent deeper. Writing the document's BOM here grew the
/// stylesheet by one U+FEFF per pass; reading the body's first byte as a BOM dropped it.
#[test]
fn svelte_nested_style_zwnbsp_is_content() {
    let head = "<svelte:head>\n\t<style>\n\t\t\u{FEFF}a {\n\t\t}\n\t</style>\n</svelte:head>\n";
    let div = "<div>\n\t<style>\n\t\t\u{FEFF}a {\n\t\t}\n\t</style>\n</div>\n";
    for (source, expected) in [
        (
            "<svelte:head><style> \u{FEFF}a{}</style></svelte:head>",
            head,
        ),
        (
            "<svelte:head><style>\u{FEFF}a{}</style></svelte:head>",
            head,
        ),
        ("<div><style>\n\t\u{FEFF}a{}</style></div>", div),
        ("<div><style>\u{FEFF}a{}</style></div>", div),
    ] {
        assert_formats_to(source, ParserType::Svelte, expected, source);
    }
}

/// The control: the top-level `<style>` island already keeps a leading U+FEFF as content,
/// with or without whitespace ahead of it.
#[test]
fn svelte_top_level_style_zwnbsp_is_content() {
    for source in ["<style> \u{FEFF}a{}</style>", "<style>\u{FEFF}a{}</style>"] {
        assert_formats_to(
            source,
            ParserType::Svelte,
            "<style>\n\t\u{FEFF}a {\n\t}\n</style>\n",
            source,
        );
    }
}

/// ◆bom_stripping stands: a BOM with no content U+FEFF behind it is not written.
#[test]
fn svelte_plain_bom_is_still_stripped() {
    assert_formats_to(
        "\u{FEFF}<div></div>\n",
        ParserType::Svelte,
        "<div></div>\n",
        "svelte plain BOM",
    );
}

// --- CSS: a leading U+FEFF the printer keeps as content ---

#[test]
fn css_selector_zwnbsp_behind_a_space_gets_a_bom() {
    assert_formats_to(
        " \u{FEFF}a{}",
        ParserType::Css,
        "\u{FEFF}\u{FEFF}a {\n}\n",
        "css selector",
    );
}

#[test]
fn css_real_bom_then_selector_zwnbsp_round_trips() {
    assert_formats_to(
        "\u{FEFF}\u{FEFF}a {\n}\n",
        ParserType::Css,
        "\u{FEFF}\u{FEFF}a {\n}\n",
        "css BOM + selector U+FEFF",
    );
}

/// A U+FEFF ahead of a leading comment reaches output byte 0 the same way.
#[test]
fn css_zwnbsp_before_a_comment_gets_a_bom() {
    assert_formats_to(
        " \u{FEFF}/* c */a{}",
        ParserType::Css,
        "\u{FEFF}\u{FEFF}/* c */\na {\n}\n",
        "css comment",
    );
}

#[test]
fn css_plain_bom_is_still_stripped() {
    assert_formats_to(
        "\u{FEFF}a {\n}\n",
        ParserType::Css,
        "a {\n}\n",
        "css plain BOM",
    );
}

// --- TypeScript: the null control ---

/// `<ZWNBSP>` is ECMAScript `WhiteSpace`: dropped as trivia, no BOM written.
#[test]
fn ts_leading_zwnbsp_is_whitespace() {
    assert_formats_to(" \u{FEFF}x", ParserType::TypeScript, "x;\n", "ts");
    assert_formats_to(
        "\u{FEFF}\u{FEFF}x",
        ParserType::TypeScript,
        "x;\n",
        "ts BOM + ZWNBSP",
    );
}
