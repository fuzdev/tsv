// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Every non-of `for await` head rejects **on the `await` keyword**.
//!
//! `for await` heads exactly one production — `ForInOfStatement`'s
//! `for await ( … of AssignmentExpression ) Statement` — so a for-in head, a C-style head
//! and a malformed head are all syntax errors. The verdict is `parse_for_standard` /
//! `parse_for_in` rejecting at entry; the *position* is the separate claim this file
//! pins, because a malformed head reaches its `;` before either builder is called and
//! that error would report at the stray token instead.
//!
//! The distinction is a content obligation as much as a diagnostic one: `await` is
//! printed off the `ForOfStatement`'s own flag, so a head accepted here would format to
//! `for (x in o)` with the keyword gone. acorn spells the same bar as an
//! `unexpected(awaitAt)` at each of its exits and reports at the keyword; matching that is
//! the claim.
//!
//! Not fixturable: `input_invalid_*` asserts only that both parsers reject, never *where*
//! — the rejections themselves are pinned by
//! `tests/fixtures/typescript/declarations/function/async/for_await/`.

use bumpalo::Bump;

/// The rendered error's `(line, column)`, read back off the caret form. A positionless
/// render carries no located line, so this panics rather than silently reading the
/// message line as a location.
fn error_at(source: &str) -> (usize, usize, String) {
    let arena = Bump::new();
    let err = tsv_ts::parse(source, &arena).expect_err("the input must not parse");
    let rendered = err.to_string();
    let mut lines = rendered.lines();
    let message = lines
        .next()
        .expect("a rendered error opens with its message")
        .to_string();
    let located = lines
        .next()
        .expect("a rendered error carries a `line:col source` line");
    let (position, _) = located.split_once(' ').unwrap_or((located, ""));
    let (line, column) = position
        .split_once(':')
        .expect("the located line is `line:col`");
    (
        line.parse().expect("line number"),
        column.parse().expect("column number"),
        message,
    )
}

/// The 1-indexed column of `needle`'s first byte on a single-line source.
fn column_of(source: &str, needle: &str) -> usize {
    source.find(needle).expect("the needle is in the source") + 1
}

const MESSAGE: &str = "'for await' can only be used in for-of loops";

/// One case per exit of the head parse that is not a for-of: the two builders' own
/// entries, and the two C-style exits that pass a separator first.
#[test]
fn every_non_of_for_await_head_reports_on_the_await_keyword() {
    for source in [
        // `parse_for_in` at entry — a binding head and an expression head
        "async function f() { for await (var x in o) {} }",
        "async function f() { for await (x in o) {} }",
        // `parse_for_standard` at entry — a well-formed C-style head reaches it directly
        "async function f() { for await (;;) {} }",
        "async function f() { for await (x; y; z) {} }",
        "async function f() { for await (var x = 1; y; z) {} }",
        // the two C-style exits, reached with a MALFORMED head: the `;` these pass would
        // fail first and report at the stray token, so only an earlier ask lands here
        "async function f() { for await (x y) {} }",
        "async function f() { for await (var x y) {} }",
    ] {
        let (line, column, message) = error_at(source);
        assert_eq!(line, 1, "single-line source: {source}");
        assert_eq!(
            message, MESSAGE,
            "the for-await bar must be the error raised, not a separator's: {source}"
        );
        assert_eq!(
            column,
            column_of(source, "await"),
            "the caret must sit on the `await` keyword, as acorn's does: {source}"
        );
    }
}

/// The control: a real for-await-of head parses, at both goals, so the bar above is not
/// simply rejecting every `for await`.
#[test]
fn a_for_await_of_head_still_parses() {
    for source in [
        "async function f() { for await (const x of o) {} }",
        "async function f() { for await (x of o) {} }",
    ] {
        let arena = Bump::new();
        tsv_ts::parse(source, &arena)
            .unwrap_or_else(|e| panic!("a for-await-of head must parse: {source} — {e}"));
    }
}
