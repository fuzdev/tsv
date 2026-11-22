// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! An expression tag's island is parsed over the rest of the document and ends at the `}` its
//! parse stops at (`tsv_ts::__private::parse_expression_to_brace`), and every island that
//! reading refuses takes the brace scan and the bounded slice parse instead. Each shape below
//! sits on one side of that line, and each must read exactly as the bounded parse reads it:
//! the tag's extent, the comment extents before its `}`, the formatted output, and an error's
//! message and position.
//!
//! - **A `/` token** is refused, so the scan decides where the island ends: a division
//!   (`{a / b}`, `{x /= 2}`), a regex holding a `}` (`a={/re}/.test(x)}`), the one the scan
//!   and the parse read apart, `{function () {} / 2}` (a division to the parse, a regex to
//!   acorn's island running through the tag's `}` — Svelte's unclosed tag).
//! - **A `/` only inside a string, template or comment** is no token, and the parse's answer
//!   stands.
//! - **A comment before the `}`** ends where it ends in the slice: the parse stops with the `}`
//!   current and reads nothing past it.
//! - **An unclosed or unfinished island** is refused and reported by the bounded path, at the
//!   same message and point.
//! - **A lookahead that reads past the `}`** (a `<` or `(` whose closer is in the document
//!   beyond the island) changes nothing.

/// The parse's wire JSON for `source`.
fn wire(source: &str) -> String {
    tsv_svelte::parse_to_json(source).expect("the document parses")
}

/// The formatted document.
fn formatted(source: &str) -> String {
    tsv_svelte::format(source).expect("the document formats")
}

/// The rendered error for `source`: its message line and its `line:col` header.
fn error(source: &str) -> (String, String) {
    let rendered = format!(
        "{:#}",
        tsv_svelte::__private::parse(source).expect_err("the document must not parse")
    );
    let mut lines = rendered.lines();
    let message = lines.next().expect("a message line").to_owned();
    let header = lines
        .next()
        .and_then(|located| located.split(' ').next())
        .expect("a located `line:col` line")
        .to_owned();
    (message, header)
}

/// Every `(source, needle)`: the wire holds `needle` — the tag's or a comment's extent.
#[track_caller]
fn wire_holds(cases: &[(&str, &str)]) {
    for &(source, needle) in cases {
        let json = wire(source);
        assert!(
            json.contains(needle),
            "{source:?}: no {needle:?} in\n{json}"
        );
    }
}

#[test]
fn a_slash_token_island_ends_where_the_scan_ends_it() {
    wire_holds(&[
        (
            "<p>{a / b}</p>",
            r#""type":"ExpressionTag","start":3,"end":10"#,
        ),
        (
            "<p>{x /= 2}</p>",
            r#""type":"ExpressionTag","start":3,"end":11"#,
        ),
        (
            "<div a={/re}/.test(x)}></div>",
            r#""type":"ExpressionTag","start":7,"end":22"#,
        ),
        (
            "<p>{a / b}</p><p>/</p>",
            r#""type":"ExpressionTag","start":3,"end":10"#,
        ),
    ]);
    assert_eq!(formatted("<p>{a / b}</p>"), "<p>{a / b}</p>\n");
    assert_eq!(
        formatted("<div a={/re}/.test(x)}></div>"),
        "<div a={/re}/.test(x)}></div>\n"
    );
}

#[test]
fn a_slash_the_scan_reads_as_a_regex_falls_back() {
    let unclosed = ("Unclosed expression tag".to_owned(), "1:4".to_owned());
    assert_eq!(error("<p>{function () {} / 2}</p>"), unclosed);
    assert_eq!(error("<p>{class {} / 2}</p>"), unclosed);
}

#[test]
fn a_slash_inside_a_string_template_or_comment_is_no_token() {
    wire_holds(&[
        (
            "<p>{'a/b}'}</p>",
            r#""type":"ExpressionTag","start":3,"end":11"#,
        ),
        (
            "<p>{`a/${b}/}`}</p>",
            r#""type":"ExpressionTag","start":3,"end":15"#,
        ),
        (
            "<p>{a /* / } */}</p>",
            r#""type":"ExpressionTag","start":3,"end":16"#,
        ),
    ]);
}

#[test]
fn a_comment_before_the_close_keeps_its_extent() {
    wire_holds(&[
        (
            "<p>{a /* c */}</p>",
            r#""type":"Block","value":" c ","start":6,"end":13"#,
        ),
        (
            "<p>{a // c\n}</p>",
            r#""type":"Line","value":" c","start":6,"end":10"#,
        ),
        (
            "<p>{a // c \n}</p>",
            r#""type":"Line","value":" c ","start":6,"end":11"#,
        ),
        (
            "<p>{a // c\n}</p>",
            r#""type":"ExpressionTag","start":3,"end":12"#,
        ),
    ]);
}

#[test]
fn an_unfinished_island_reports_as_the_bounded_parse_does() {
    let cases = [
        ("<p>{a + b</p>", "Unclosed expression tag", "1:4"),
        ("<p>{a b}</p>", "Expected '}', found identifier", "1:7"),
        // the slice's end, not the `}` the island's own parse stopped at
        (
            "<p>{a +}</p>",
            "Expected expression, found end of input",
            "1:8",
        ),
        (
            "<p>{}</p>",
            "Expected expression, found end of input",
            "1:5",
        ),
    ];
    for (source, message, header) in cases {
        assert_eq!(
            error(source),
            (message.to_owned(), header.to_owned()),
            "{source:?}"
        );
    }
}

#[test]
fn a_lookahead_past_the_close_changes_nothing() {
    wire_holds(&[
        (
            "<p>{a < b}</p><b>x</b> > (y) => z",
            r#""type":"BinaryExpression","start":4,"end":9"#,
        ),
        (
            "<p>{(a)}</p> => b",
            r#""type":"ExpressionTag","start":3,"end":8"#,
        ),
        (
            "<p>{f<T>}</p><b>(x)</b>",
            r#""type":"ExpressionTag","start":3,"end":9"#,
        ),
        (
            "<p>{a ? (b) : c}</p> => d",
            r#""type":"ConditionalExpression","start":4,"end":15"#,
        ),
    ]);
    assert_eq!(
        formatted("<p>{a < b}</p><b>x</b> > (y) => z"),
        "<p>{a < b}</p>\n<b>x</b> > (y) => z\n"
    );
}

/// A backtick inside a template's interpolation pairs the wrong two backticks for the parser's
/// byte lookaheads, which skip a template from one backtick to the next, and the quote it
/// leaves opens a string that runs over the island's `}` into the document — so what a
/// lookahead finds there would depend on the document. Such an island takes the bounded
/// parse, and reads as it does there whatever follows it. (The bounded reading itself is the
/// lookahead's misread — `f<A, `${"`"}`>(c)` is a call to tsc — which this pins as it stands.)
#[test]
fn a_backtick_inside_an_interpolation_takes_the_bounded_parse() {
    for source in [
        "<p>{f<A, `${\"`\"}`>(c)}</p>",
        "<p>{f<A, `${\"`\"}`>(c)}\"></p>",
    ] {
        let json = wire(source);
        let needle = r#""type":"ExpressionTag","start":3,"end":22,"expression":{"type":"SequenceExpression","start":4,"end":21"#;
        assert!(
            json.contains(needle),
            "{source:?}: no {needle:?} in\n{json}"
        );
    }
    assert_eq!(
        error("<p>{(x = `${\"`\"}`) => x}\") =></p>"),
        ("Expected '}', found '=>'".to_owned(), "1:20".to_owned())
    );
}

/// A division after a braced operand inside an arrow head's parens: the lookahead that finds
/// the head's `)` reads the `/` after the `}` as the division it is, so no "regex" runs over the
/// island's `}` to a `/` in the document, past a `)` and `=>` there — each island parses, and
/// ends where it does, whatever follows it.
#[test]
fn a_division_after_a_braced_operand_reads_the_same_whatever_follows() {
    wire_holds(&[
        (
            "<p>{(a = {} / 2) => a}</p>",
            r#""type":"ExpressionTag","start":3,"end":22"#,
        ),
        (
            "<p>{(a = {} / 2) => a}</p>)=>1/",
            r#""type":"ExpressionTag","start":3,"end":22"#,
        ),
        (
            "{(x = {} / 2) => a}/) => q",
            r#""type":"ExpressionTag","start":0,"end":19"#,
        ),
        (
            "<div a={(a = function () {} / 2) => a}></div>)=>1/",
            r#""type":"ExpressionTag","start":7,"end":38"#,
        ),
    ]);
}
