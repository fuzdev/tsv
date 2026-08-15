// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Lexer error positions are **host** coordinates — the document the error is rendered
//! against.
//!
//! A `Lexer` is routinely built over a *slice*: a Svelte `<script>` / `<style>` island,
//! the CSS declaration-value scan (`source[from..]`), the Svelte parser's own reseek
//! after a jumped scan. `ErrorContext::from_source` is always handed the WHOLE document,
//! so a position in any other coordinate space points at the wrong construct — a
//! component whose script fails on line 4 reported at `1:13`, out in the markup, and a
//! plain `.css` file whose declaration value fails at column 15 reported at column 3.
//!
//! The parser side has always shifted (`Parser::current_pos` adds `base_offset`), so
//! *parser* errors are correct and only *lexer* errors drift. That asymmetry is why a
//! standalone `.ts` test can't see this: the same input as a `.ts` file reports
//! correctly.
//!
//! Not fixturable: no fixture type asserts an error POSITION — `input_invalid_*` asserts
//! only that both parsers reject, `tsv_rejects.txt` asserts a message substring — so the
//! relation lives here.
//!
//! Every assertion is also a **double-shift** control. Shifting a position twice puts it
//! past the end of the source, `ErrorContext::from_source` returns `None`, and the render
//! collapses to the caret-free `… at position N` form, which [`location`] rejects rather
//! than reads.

use bumpalo::Bump;

/// The rendered error's `(line, column, source line)`, read back off the caret form.
///
/// A positionless render carries no located line at all, so this panics rather than
/// silently reading the message line as a location.
fn location(rendered: &str) -> (usize, usize, String) {
    let mut lines = rendered.lines();
    lines
        .next()
        .expect("a rendered error opens with its message");
    let located = lines.next().expect(
        "a rendered error carries a `line:col source` line — a position outside the source \
         renders the caret-free fallback instead",
    );
    let (position, text) = located.split_once(' ').unwrap_or((located, ""));
    let (line, column) = position
        .split_once(':')
        .expect("the located line is `line:col`");
    (
        line.parse().expect("line number"),
        column.parse().expect("column number"),
        text.to_string(),
    )
}

fn svelte_error(source: &str) -> (usize, usize, String) {
    let arena = Bump::new();
    let err = tsv_svelte::parse(source, &arena).expect_err("the input must not parse");
    location(&err.to_string())
}

fn ts_error(source: &str) -> (usize, usize, String) {
    let arena = Bump::new();
    let err = tsv_ts::parse(source, &arena).expect_err("the input must not parse");
    location(&err.to_string())
}

fn css_error(source: &str) -> (usize, usize, String) {
    let arena = Bump::new();
    let err = tsv_css::parse(source, &arena).expect_err("the input must not parse");
    location(&err.to_string())
}

/// `(line, column)` of `needle`'s first byte, both 1-indexed — where the caret belongs.
fn token_at(source: &str, needle: &str) -> (usize, usize) {
    let at = source.find(needle).expect("the needle is in the source");
    let line_start = source[..at].rfind('\n').map_or(0, |i| i + 1);
    (source[..at].matches('\n').count() + 1, at - line_start + 1)
}

/// The line the caret must point at, for the readable half of an assertion.
fn line_text(source: &str, line: usize) -> String {
    source
        .lines()
        .nth(line - 1)
        .expect("the expected line exists")
        .to_string()
}

/// Markup above an island, ending flush with the tag's own line: the island's first line
/// is the *tail* of the `<script>` line, so island line `n` is host line `n + LINES_ABOVE`
/// at the same column.
const MARKUP: &str = "<div>hi</div>\n\n";
const LINES_ABOVE: usize = 2;

/// An island error lands where the identical standalone parse lands, translated into host
/// coordinates — one case per error-producing lexer entry point, since each is its own
/// return path.
#[test]
fn script_island_lexer_errors_land_where_the_standalone_parse_lands() {
    for body in [
        "\n\tconst s = \"unterminated;\n", // next_token_into
        "\n\tconst t = `a${b}c;\n",        // continue_template_from_brace
        "\n\tconst r = /unterminated;\n",  // read_regex_literal
    ] {
        let host = format!("{MARKUP}<script>{body}</script>\n");

        let (island_line, island_column, island_text) = ts_error(body);
        assert!(
            island_line >= 2,
            "the body must fail below its own first line for the translation to be a \
             line shift alone: {body:?} failed at line {island_line}"
        );

        assert_eq!(
            svelte_error(&host),
            (island_line + LINES_ABOVE, island_column, island_text),
            "island error must be the standalone error in host coordinates: {body:?}"
        );
    }
}

/// The concrete shape of the bug this file exists for, spelled out: the string opens on
/// line 4 of the component, not at `1:13` in the markup.
#[test]
fn script_island_string_error_points_at_the_quote_on_its_own_line() {
    let host = format!("{MARKUP}<script>\n\tconst s = \"unterminated;\n</script>\n");
    let (line, column) = token_at(&host, "\"unterminated");

    assert_eq!(line, 4, "the offending quote is on line 4");
    assert_eq!(svelte_error(&host), (line, column, line_text(&host, line)),);
}

/// `<style>` islands run the same relation through the CSS lexer.
#[test]
fn style_island_lexer_errors_land_where_the_standalone_parse_lands() {
    let body = "\n\t.a {\n\t\tcontent: \"bad;\n\t}\n";
    let host = format!("{MARKUP}<style>{body}</style>\n");

    let (island_line, island_column, island_text) = css_error(body);
    assert!(
        island_line >= 2,
        "the body must fail below its own first line"
    );

    assert_eq!(
        svelte_error(&host),
        (island_line + LINES_ABOVE, island_column, island_text),
    );
}

/// The Svelte template lexer is rebuilt over `source[pos..]` whenever the parser jumps
/// the cursor (`advance_to_position`), so without the slice's own offset its errors carry
/// the offset of whatever island preceded them. The pair is the control: **the same
/// failing line reports the same column whether or not a reseek preceded it.**
#[test]
fn template_lexer_errors_do_not_depend_on_a_preceding_reseek() {
    let plain = "<p>hello</p>\n<div class=\"unterminated\n";
    let after_reseek = "{@html x}\n<div class=\"unterminated\n";

    let expected_column = token_at(plain, "\"unterminated").1;
    assert_eq!(
        svelte_error(plain),
        (2, expected_column, line_text(plain, 2)),
    );
    assert_eq!(
        svelte_error(after_reseek),
        (2, expected_column, line_text(after_reseek, 2)),
        "a reseek moves the lexer's slice, not the reported position"
    );
}

/// An island the parser skips wholesale (raw text, `<style>`) is the same reseek class.
#[test]
fn template_lexer_error_after_a_skipped_island_points_at_its_own_line() {
    let source = "<textarea>x</textarea>\n<!-- unterminated\n";
    let (line, column) = token_at(source, "<!--");

    assert_eq!(
        svelte_error(source),
        (line, column, line_text(source, line)),
    );
}

/// The CSS declaration-value scan lexes `source[from..]`, where `from` is inside the
/// declaration — so this one is wrong in a plain `.css` file, with no embedding
/// anywhere. The pair varies the property name, i.e. the slice offset itself: the caret
/// must land on the quote in both.
#[test]
fn css_declaration_value_errors_do_not_depend_on_the_scan_slice() {
    for source in [".b { c: \"bad; }", ".b { content: \"bad; }"] {
        let (line, column) = token_at(source, "\"bad");
        assert_eq!(
            css_error(source),
            (line, column, line_text(source, line)),
            "the caret must sit on the quote, not at the scan's own offset: {source:?}"
        );
    }
}

/// The same declaration across lines, where the scan's offset costs the line number as
/// well as the column.
#[test]
fn css_declaration_value_error_reports_its_own_line() {
    let source = ".b {\n\tcontent: \"bad;\n}\n";
    let (line, column) = token_at(source, "\"bad");

    assert_eq!(line, 2, "the offending quote is on line 2");
    assert_eq!(css_error(source), (line, column, line_text(source, line)),);
}

/// The null control for every case above: a standalone parse has no offset to add, and
/// its positions must not move. A shift applied unconditionally (or twice) fails here
/// first.
#[test]
fn standalone_lexer_errors_are_unchanged() {
    let ts = "const a = 1;\nconst b = 2;\nconst s = \"bad;\n";
    let (line, column) = token_at(ts, "\"bad");
    assert_eq!(ts_error(ts), (line, column, line_text(ts, line)));

    let css = "/* unterminated\n";
    assert_eq!(css_error(css), (1, 1, line_text(css, 1)));
}
