// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A block glued to a redundant grouping `(` keeps the pair when the pair's interior opens
//! with a line comment, and "opens with" skips whitespace by the LEXER's class: a non-ASCII
//! space (NBSP), a vertical tab or a line terminator (`<LS>`) between the `(` and the `//` is
//! whitespace like any other. Read narrower, the pair strips on the first pass — and where
//! the position re-emits the pair anyway (a chain base, an IIFE callee, an `as` operand), the
//! printed `( // d` is ASCII, so the second pass keeps the pair the first did not: an F1
//! failure. Not a fixture: a fixture input is a format fixed point, so it cannot carry the
//! non-ASCII space the first pass rewrites.

fn format(source: &str) -> String {
    tsv_ts::format(source).expect("format failed")
}

#[test]
fn non_ascii_space_before_the_line_comment_is_lexer_whitespace() {
    for space in ["\u{a0}", "\u{b}", "\u{feff}", "\u{2028}", "\u{3000}"] {
        for (head, tail) in [
            ("v = /* c */ (", "a?.b\n).c;\n"),
            ("/* c */ (", "() => 1\n)();\n"),
            ("v = /* c */ (", "a\n) as T;\n"),
            ("const v = /* c */ (", "g()\n);\n"),
        ] {
            let source = format!("{head}{space}// d\n{tail}");
            let once = format(&source);
            assert!(
                once.contains("/* c */ ("),
                "pair stripped for {source:?}: {once:?}"
            );
            assert_eq!(format(&once), once, "not a fixed point for {source:?}");
        }
    }
}
