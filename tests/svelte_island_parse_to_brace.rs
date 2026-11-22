// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! An expression tag's island is parsed over the rest of the document and ends at the `}` its
//! parse stops at (`tsv_ts::__private::parse_expression_to_brace`), and every island that
//! reading refuses takes the brace scan and the bounded slice parse instead. Each shape below
//! sits on one side of that line, and each must read exactly as the bounded parse reads it:
//! the tag's extent, the comment extents before its `}`, the formatted output, and an error's
//! message and position.
//!
//! - **A `/` the scans and the parse may read apart** is refused, so the scan decides where
//!   the island ends: a division after a `}` — `{function () {} / 2}` is a division to the
//!   parse and a regex to acorn's island scan, running through the tag's `}` (Svelte's
//!   unclosed tag) — a regex after a word or a `)`, and a `/` after whitespace the scans take
//!   for code (a vertical tab, a non-ASCII space: `{aa\u{FEFF}/ bb}` divides to the parse and
//!   opens a regex to the scans).
//! - **Every other `/`** — a division after a name, a number, a `)` or `]`
//!   (`{a / b}`, `{x /= 2}`), a regex after an operator or at the island's start
//!   (`a={/re}/.test(x)}`) — is read alike by the parse, its lookaheads and the island scan,
//!   and the parse's answer stands; it ends where the scan would have ended it.
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
fn a_slash_read_alike_ends_where_the_scan_ends_it() {
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
        (
            "<p>{(a + b) / c[0] / 2}</p><p>/) => 1/</p>",
            r#""type":"ExpressionTag","start":3,"end":23"#,
        ),
        (
            "<p>{x.replace(/}/g, '') / 'a'.length}</p>",
            r#""type":"ExpressionTag","start":3,"end":37"#,
        ),
        (
            "<p>{(a = b / 2) => a}</p><p>/) => 1/</p>",
            r#""type":"ExpressionTag","start":3,"end":21"#,
        ),
        (
            "<p>{`${a}/${b}` / 2}</p>",
            r#""type":"ExpressionTag","start":3,"end":20"#,
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
    // A division after an object literal is acorn's too, so the scan ends the island at the
    // same `}` the parse does.
    wire_holds(&[(
        "<p>{{} / 2}</p><p>/</p>",
        r#""type":"ExpressionTag","start":3,"end":11"#,
    )]);
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

/// A backtick inside a template's interpolation: the parser's byte lookaheads skip a template
/// literal whole, its interpolations included, so no quote is left to open a string that runs
/// over the island's `}` into the document — the type-argument list is a call's, and the arrow
/// head an arrow's, whatever follows the tag.
#[test]
fn a_backtick_inside_an_interpolation_reads_the_same_whatever_follows() {
    for source in [
        "<p>{f<A, `${\"`\"}`>(c)}</p>",
        "<p>{f<A, `${\"`\"}`>(c)}\"></p>",
    ] {
        let json = wire(source);
        let needle = r#""type":"ExpressionTag","start":3,"end":22,"expression":{"type":"CallExpression","start":4,"end":21"#;
        assert!(
            json.contains(needle),
            "{source:?}: no {needle:?} in\n{json}"
        );
    }
    wire_holds(&[(
        "<p>{(x = `${\"`\"}`) => x}\") =></p>",
        r#""type":"ExpressionTag","start":3,"end":24"#,
    )]);
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

/// A template inside one of the grammar's answers — the `>` before a `/` — is read under the
/// grammar too, its object literal's `}` and type-argument list's `>` dividing as they do
/// outside it, so the island parses, and ends where it does, whatever follows the tag.
#[test]
fn a_template_inside_a_type_argument_answer_reads_the_same_whatever_follows() {
    for tail in ["", "<p>/) => 1/</p>"] {
        wire_holds(&[
            (
                &format!("<script lang=\"ts\"></script><p>{{`${{{{}} / 2}}` + f<T>/2}}</p>{tail}"),
                r#""type":"ExpressionTag","start":30,"end":52"#,
            ),
            (
                &format!("<script lang=\"ts\"></script><p>{{`${{g<T>/2}}` + f<T>/2}}</p>{tail}"),
                r#""type":"ExpressionTag","start":30,"end":52"#,
            ),
            (
                &format!(
                    "<script lang=\"ts\"></script><p>{{(a = `${{{{}} / 2}}`, b = f<T>/2) => a}}</p>{tail}"
                ),
                r#""type":"ExpressionTag","start":30,"end":66"#,
            ),
            // The answer's own walk reads a `}` as the grammar does: the object literal's `/`
            // divides, so the quoted `/` after it opens no regex running over the list's `>`.
            (
                &format!(
                    "<script lang=\"ts\"></script><p>{{`${{{{}} / 2 + '/' + f<T>/2}}`}}</p>{tail}"
                ),
                r#""type":"ExpressionTag","start":30,"end":58"#,
            ),
            (
                &format!(
                    "<script lang=\"ts\"></script><p>{{(a = `${{{{}} / 2 + '/' + f<T>/2}}`) => a}}</p>{tail}"
                ),
                r#""type":"ExpressionTag","start":30,"end":69"#,
            ),
        ]);
    }
}

/// Whether the island at byte 1 of `source` (past its `{`) is answered by its own parse.
fn closed_by_parse(source: &str) -> bool {
    tsv_ts::__private::parse_expression_to_brace(source, 1, 0, &mut tsv_ts::ast::AstStores::new())
        .is_some()
}

#[test]
fn the_parse_answers_a_slash_read_alike_and_refuses_the_rest() {
    for source in [
        "{a / b}",
        "{x /= 2}",
        "{(a + b) / 2}",
        "{a[0] / 'b'.length}",
        "{a++ / 2}",
        "{/re/.test(x)}",
        "{a && /re/.test(x)}",
        "{!/re/.test(x)}",
        "{(a) => /re/.test(a)}",
        "{`${a}/${b}`}",
        "{f<A, `${'`'}`>(c)}",
    ] {
        assert!(closed_by_parse(source), "{source:?}");
    }
    for source in [
        "{{} / 2}",
        "{function () {} / 2}",
        "{a! / 2}",
        "{a.await / 2}",
        "{() => { if (c) /re/.test(s) }}",
        "{() => { for (x of /re/g.exec(s)); }}",
        "{() => { return /re/ }}",
    ] {
        assert!(!closed_by_parse(source), "{source:?}");
    }
}

/// Every whitespace character JS reads as trivia that the scans' anchor walk does not
/// (`is_ascii_whitespace`): the vertical tab and each non-ASCII `WhiteSpace` /
/// `LineTerminator`. The walk reads one as a significant byte, which is no operand's end, so
/// a `/` after it opens a regex to the scans.
const GAP_WHITESPACE: &[char] = &[
    '\u{000B}', '\u{00A0}', '\u{FEFF}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}',
    '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{2028}',
    '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}',
];

#[test]
fn a_slash_after_whitespace_the_scans_read_as_code_is_refused() {
    // `G` is the gap character, between the token before the `/` and the `/`, beside
    // comments of each kind.
    let divisions = [
        "{aaG/ bb}",
        "{aa G / bb}",
        "{aa /* c */G/ bb}",
        "{aaG/* c */ / bb}",
        "{aa // c\nG/ bb}",
        "{aa++G/ 2}",
        "{a--G/ 2}",
        "{(a)G/ 2}",
        "{a[0]G/ 2}",
        "{'s'G/ 2}",
        "{1G/ 2}",
        "{xG/= 2}",
    ];
    let regexes = [
        "{a &&G/re/.test(x)}",
        "{a && /* c */G/re/.test(x)}",
        "{!G/re/.test(x)}",
        "{aG!/re/.test(x)}",
        "{a +G/re/.test(x)}",
        "{a +G+/re/.test(x)}",
        "{(a) =>G/re/.test(a)}",
        "{f(G/re/)}",
        "{G/re/.test(x)}",
    ];
    for &gap in GAP_WHITESPACE {
        for template in divisions.iter().chain(&regexes) {
            let island = template.replace('G', &gap.to_string());
            // A debug build asserts that an island its parse answers ends where the brace
            // scan ends it; the document's own verdict is not the question here.
            let _ = tsv_svelte::__private::parse(&format!("<p>{island}</p><p>/</p>"));
        }
        for template in divisions {
            let island = template.replace('G', &gap.to_string());
            assert!(!closed_by_parse(&island), "{island:?}");
        }
    }
    // ASCII whitespace and comments in the gap are read alike.
    for island in [
        "{aa \t\r\n\x0C/ bb}",
        "{aa /* c\u{FEFF} */ / bb}",
        "{aa // c\u{FEFF}\n/ bb}",
        "{aa // c\u{2028}/ bb}",
        "{a && /* c */ /re/.test(x)}",
    ] {
        assert!(closed_by_parse(island), "{island:?}");
    }
}

#[test]
fn a_regex_after_a_bang_glued_to_a_word_is_refused() {
    // The scans step over a `!` run glued to a word as a postfix non-null and read the `/`
    // after `of` as a division; the parse reads `of`, a prefix `!`, then a regex. The host
    // text after the island holds the quote or brace a misread scan would run on to.
    for island in [
        "{(() => { for (const x of!/'/.exec(s)) ; })()}",
        "{(() => { for (const x of /* c */!/'/.exec(s)) ; })()}",
        "{(() => { for (const x of!!/}/.exec(s)) ; })()}",
    ] {
        // A debug build asserts that an island its parse answers ends where the brace scan
        // ends it.
        let _ = tsv_svelte::__private::parse(&format!("<p>{island}</p><p>'/}}</p>"));
        assert!(!closed_by_parse(island), "{island:?}");
    }
}
