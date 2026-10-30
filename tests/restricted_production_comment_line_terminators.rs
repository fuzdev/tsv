// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A block comment ahead of a restricted production's argument whose interior holds a
//! `<LS>` (U+2028) or `<PS>` (U+2029) is a line terminator for ASI (ecma262 sec-comments: a
//! `MultiLineComment` containing a `LineTerminator` is one), exactly as one holding a `\n` is.
//! Printed bare after `return` / `throw` / `yield`, it ends the statement there and the
//! argument becomes a statement of its own; the argument takes the hanging pair instead.
//!
//! These stay unit tests rather than fixtures because the bytes do: no fixture carries a
//! `<LS>` / `<PS>`, and the comment's own `multiline` flag reads `\n` alone (Svelte's wire
//! gate), so the reading under test is the leading-edge walker's, not the flag's — and, for a
//! sequence argument, which prints its own `(` ahead of the comment, the arm chooser's.

/// Format a TypeScript document.
fn format(source: &str) -> String {
    tsv_ts::format(source).expect("format failed")
}

/// Wrap `statement` in the function its keyword needs.
fn wrap(statement: &str) -> String {
    if statement.starts_with("yield") {
        format!("function* g() {{\n\t{statement}\n}}\n")
    } else {
        format!("function f() {{\n\t{statement}\n}}\n")
    }
}

/// Each row: the keyword, and the terminator inside the comment.
const ROWS: &[(&str, char)] = &[
    ("return", '\u{2028}'),
    ("return", '\u{2029}'),
    ("throw", '\u{2028}'),
    ("throw", '\u{2029}'),
    ("yield", '\u{2028}'),
    ("yield*", '\u{2029}'),
];

#[test]
fn line_separator_in_a_leading_block_takes_the_hanging_pair() {
    let mut failures = Vec::new();
    for &(keyword, terminator) in ROWS {
        let comment = format!("/* a{terminator}b */");
        // Glued inside the statement's own grouping parens, owned by the argument. (Written
        // in the keyword gap bare, the comment already ends a `return` / `throw` / `yield` at
        // parse; `yield*` has no such restriction after its `*`, which the sequence test
        // below spells bare.)
        let shell = wrap(&format!("{keyword} ({comment} x);"));
        // a chain whose leftmost shell holds it
        let left = wrap(&format!("{keyword} (({comment} a).b + 1);"));
        let cases = [
            (shell, wrap(&format!("{keyword} (\n\t\t{comment} x\n\t);"))),
            (
                left,
                wrap(&format!("{keyword} (\n\t\t{comment} a.b + 1\n\t);")),
            ),
        ];
        for (authored, expected) in cases {
            let once = format(&authored);
            if once != expected {
                failures.push(format!("{authored:?} printed {once:?}"));
            } else if format(&once) != once {
                failures.push(format!("{authored:?} is not its own fixed point"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn line_separator_ahead_of_a_sequence_takes_the_hanging_pair_on_every_pass() {
    // A sequence prints its own `(`, so once a pass has moved the comment inside it the
    // built doc no longer shows it ahead of the argument: the arm must be chosen for it, as
    // for the `\n` spelling, or the second pass prints the bare sequence.
    let mut failures = Vec::new();
    for keyword in ["return", "throw", "yield", "yield*"] {
        for terminator in ['\u{2028}', '\u{2029}'] {
            let comment = format!("/* a{terminator}b */");
            let expected = wrap(&format!("{keyword} (\n\t\t{comment} a, zz\n\t);"));
            let mut authored = vec![
                wrap(&format!("{keyword} ({comment} a, zz);")),
                wrap(&format!("{keyword} ({comment} (a, zz));")),
            ];
            // `yield*` takes no `[no LineTerminator here]` after its `*`, so the comment
            // may sit in the keyword gap bare
            if keyword == "yield*" {
                authored.push(wrap(&format!("{keyword} {comment} (a, zz);")));
            }
            for authored in authored {
                let once = format(&authored);
                if once != expected {
                    failures.push(format!("{authored:?} printed {once:?}"));
                } else if format(&once) != once {
                    failures.push(format!("{authored:?} is not its own fixed point"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The null control: a single-line block with no terminator stays on the keyword's line.
#[test]
fn single_line_block_stays_on_the_keyword_line() {
    for keyword in ["return", "throw", "yield", "yield*"] {
        let authored = wrap(&format!("{keyword} (/* a b */ x);"));
        assert_eq!(
            format(&authored),
            wrap(&format!("{keyword} /* a b */ x;")),
            "{keyword}"
        );
    }
}
