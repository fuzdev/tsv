// helper fns here aren't `#[test]`, so clippy.toml's allow-expect/panic-in-tests don't reach them
#![expect(clippy::expect_used, clippy::panic)]

//! A lone `<CR>` inside a `//` comment written between an element's attributes is the one
//! byte the format path's line-terminator fold cannot rewrite, so a Svelte format REFUSES
//! the document rather than change what it means.
//!
//! Every format entry point folds `<CR>` / `<CR><LF>` to `<LF>` ahead of its parse
//! (`tsv_lang::printing::normalize_carriage_returns`). Everywhere else in a Svelte document
//! that rewrite keeps meaning: a `<script>` or a `{…}` island is acorn's, which ends a line
//! at a `<CR>` too; HTML text and attribute values normalize `<CR>` to `<LF>` at the input
//! stream; a block comment only changes its own bytes. But Svelte's template reader ends an
//! in-tag `//` comment at the next `\n` ALONE, so `<div // c⏎class="x">` (`⏎` a lone `<CR>`)
//! is a `div` with no attributes whose comment runs to the next `\n`. Folding the `<CR>`
//! ends the comment early and turns the rest of its line into markup: a real `class`
//! attribute, or a parse that succeeds on a document `tsv parse` rejects.
//!
//! So when the fold rewrote a lone `<CR>` in a Svelte document, the format parses the
//! AUTHOR's bytes first: their parse error is the format's error (the one `tsv parse`
//! reports), and an in-tag `//` comment holding a lone `<CR>` is refused. A `<CR><LF>` is
//! one line break in both readings, so it never refuses — and a document the fold never
//! touched pays nothing.
//!
//! Not fixturable: the format path folds a lone `<CR>`, so no input holding one is a
//! fixed point.

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

/// The words every refusal opens with.
const REFUSAL: &str = "lone carriage return inside a '//' comment in a tag";

/// The CLI's `format_source` error and the fused `tsv_svelte::format_str` error for
/// `source` — two entry points that must agree, asserted here so each case states it once.
fn svelte_format_error(source: &str) -> String {
    let cli = format_source(source, ParserType::Svelte)
        .expect_err("the format must refuse")
        .to_string();
    let fused = tsv_svelte::format_str(source)
        .expect_err("the fused format must refuse")
        .to_string();
    assert_eq!(cli, fused, "the CLI and the fused entry point disagree");
    cli
}

/// The format of `source` through both Svelte entry points, which must agree.
fn svelte_format_ok(source: &str, label: &str) -> String {
    let cli = format_source(source, ParserType::Svelte)
        .unwrap_or_else(|e| panic!("{label} must format: {e}"));
    let fused = tsv_svelte::format_str(source).expect("the fused format must agree");
    assert_eq!(
        cli, fused,
        "{label}: the CLI and the fused entry point disagree"
    );
    cli
}

/// The rendered `tsv parse` error for `source` — what a refusal-free format failure of the
/// author's bytes must report, message and position alike.
fn svelte_parse_error(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    tsv_svelte::parse(source, &arena)
        .expect_err("the author's bytes must not parse")
        .to_string()
}

#[test]
fn a_lone_cr_in_an_in_tag_line_comment_refuses_rather_than_mint_an_attribute() {
    // The author's document is a `div` with NO attributes — Svelte's reader runs the comment
    // past the `<CR>` to the `\n` — and the fold would print a real `class` attribute.
    let source = "<div // c\rclass=\"x\"\n>hi</div>\n";
    let arena = bumpalo::Bump::new();
    tsv_svelte::parse(source, &arena).expect("the author's bytes are valid Svelte");

    let error = svelte_format_error(source);
    let mut lines = error.lines();
    let message = lines.next().expect("a message line");
    assert!(
        message.to_lowercase().contains(REFUSAL),
        "the refusal must say why: {error}"
    );
    // The `<CR>` itself, in Svelte's `\n`-only line count: line 1, column 9 (`1:10`).
    let located = lines.next().expect("a `line:col` line");
    assert!(
        located.starts_with("1:10 "),
        "the refusal must point at the `<CR>`: {error}"
    );
}

#[test]
fn a_lone_cr_in_an_in_tag_line_comment_refuses_past_a_crlf_line() {
    // A `<CR><LF>` ahead of it moves the position by no line the refusal miscounts: the
    // `<CR>` sits on line 2 at column 9, the same point `tsv parse` would give it.
    let source = "<p>a</p>\r\n<div // c\rid=\"y\"\n>hi</div>\n";
    let error = svelte_format_error(source);
    assert!(error.to_lowercase().contains(REFUSAL), "{error}");
    assert_eq!(
        error
            .lines()
            .nth(1)
            .and_then(|line| line.split_once(' '))
            .map(|(at, _)| at),
        Some("2:10"),
        "{error}"
    );
}

#[test]
fn a_document_the_authors_bytes_cannot_parse_reports_that_parse_error() {
    // Here the comment swallows the tag's `>` too, so the author's document never closes
    // its tag: `tsv parse` rejects it, and the format must say exactly what that parse says
    // rather than format the folded document it would otherwise accept.
    let source = "<div // c\rclass=\"x\">hi</div>\n";
    assert_eq!(svelte_format_error(source), svelte_parse_error(source));
}

#[test]
fn a_crlf_document_with_in_tag_line_comments_formats() {
    // `<CR><LF>` is one line break in both readings — Svelte's comment ends at its `\n`, and
    // the `<CR>` the comment then holds folds away with the pair — so this must not refuse.
    let lf = "<div // c\nclass=\"x\"\n>hi</div>\n<input // d\n\tvalue=\"v\"\n/>\n";
    let crlf = lf.replace('\n', "\r\n");
    assert_eq!(
        svelte_format_ok(&crlf, "crlf"),
        svelte_format_ok(lf, "lf"),
        "the CRLF twin formats like its LF form"
    );
    // a lone `<CR>` with only whitespace after it — the doubled `<CR><CR><LF>` ending, or a
    // trailing space run — ends the comment where the fold does, so it formats too
    let doubled = lf.replace('\n', "\r\r\n");
    assert_eq!(
        svelte_format_ok(&doubled, "crcrlf"),
        svelte_format_ok(&lf.replace('\n', "\n\n"), "lflf"),
        "the CRCRLF twin formats like the doubled-LF document its fold is"
    );
    svelte_format_ok(
        "<div // c\r \t\nclass=\"x\"\n>hi</div>\n",
        "cr then whitespace",
    );
}

#[test]
fn a_lone_cr_anywhere_else_in_a_svelte_document_still_formats() {
    for (label, source) in [
        ("text", "<p>a\rb</p>\n"),
        ("attribute value", "<div class=\"a\rb\">hi</div>\n"),
        (
            "script",
            "<script>\r\tconst a = 1; // c\r\tconst b = 2;\r</script>\n",
        ),
        ("expression", "<p>{a // c\r+ b}</p>\n"),
        (
            "in-tag block comment",
            "<div /* a\rb */ class=\"x\">hi</div>\n",
        ),
        (
            "between attributes",
            "<div\rclass=\"x\"\rid=\"y\">hi</div>\n",
        ),
    ] {
        let arena = bumpalo::Bump::new();
        assert!(
            tsv_svelte::parse(source, &arena).is_ok(),
            "{label}: the author's bytes must parse for this to be a control"
        );
        let formatted = svelte_format_ok(source, label);
        assert!(
            !formatted.contains('\r'),
            "{label}: tsv's output is LF-only"
        );
    }
}

#[test]
fn typescript_and_css_format_a_lone_cr_as_its_line_feed() {
    // Their line rule ends a `//` / a declaration at a lone `<CR>` already, so the fold is
    // meaning-preserving there: a lone `<CR>` formats exactly as the `<LF>` it folds to,
    // through both entry points — no refusal, and no second parse changing the answer.
    for (parser, source) in [
        (ParserType::TypeScript, "const a = 1; // c\rconst b = 2;\r"),
        (ParserType::Css, "a {\r\tcolor: red; /* c\rd */\r}\r"),
    ] {
        let lf = source.replace('\r', "\n");
        let format = |text: &str| {
            let cli = format_source(text, parser)
                .unwrap_or_else(|e| panic!("{parser:?} {text:?} must format: {e}"));
            let fused = match parser {
                ParserType::TypeScript => tsv_ts::format_str(text),
                ParserType::Css => tsv_css::format_str(text),
                ParserType::Svelte => tsv_svelte::format_str(text),
            }
            .unwrap_or_else(|e| panic!("{parser:?} {text:?} must format (fused): {e}"));
            assert_eq!(
                cli, fused,
                "{parser:?}: the CLI and the fused entry point disagree"
            );
            cli
        };
        assert_eq!(format(source), format(&lf), "{parser:?}: {source:?}");
    }
}
