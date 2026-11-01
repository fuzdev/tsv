// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! An array element followed by neither `,` nor `]` is reported as the missing separator,
//! the way every other list says it (`Expected ',' or '}' after list element`): the parser
//! has just read an element, so an expression is not what it wants there.
//!
//! acorn rejects each at the stray token (`Unexpected token`); the message is tsv's own.
//! The array literal is also the array pattern's parse (`let [a b] = c`, `[a b] = c`,
//! `for ([a b] of c)`), so one message covers both.
//!
//! Not fixturable: `input_invalid_*` asserts only that both parsers reject, never the
//! message or its point.

/// The rendered error for `source`, parsed as a module: its message line and its
/// `line:col` header.
fn error(source: &str) -> (String, String) {
    let arena = bumpalo::Bump::new();
    let rendered = format!(
        "{:#}",
        tsv_ts::__private::parse_with_goal(source, tsv_ts::Goal::Module, &arena)
            .expect_err("the array must not parse")
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

/// Every `(source, message, header)` case, failures collected so one run names them all.
#[track_caller]
fn check(cases: &[(&str, &str, &str)]) {
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|&(source, message, header)| {
            let actual = error(source);
            let expected = (message.to_owned(), header.to_owned());
            (actual != expected).then(|| {
                format!("{source:?}\n    tsv:      {actual:?}\n    expected: {expected:?}")
            })
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} case(s) differ:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

const ARRAY: &str = "Expected ',' or ']' after list element";

#[test]
fn an_array_element_without_a_separator_names_the_separator() {
    check(&[
        ("[1 2]", &format!("{ARRAY}, found number"), "1:4"),
        ("[1, 2 3]", &format!("{ARRAY}, found number"), "1:7"),
        ("[...a b]", &format!("{ARRAY}, found identifier"), "1:7"),
        ("f([1 2])", &format!("{ARRAY}, found number"), "1:6"),
        ("[1", &format!("{ARRAY}, found end of input"), "1:3"),
    ]);
}

/// The array pattern is read as an array literal first, so it takes the same message.
#[test]
fn an_array_pattern_element_without_a_separator_names_the_separator() {
    check(&[
        (
            "let [a b] = c;",
            &format!("{ARRAY}, found identifier"),
            "1:8",
        ),
        ("[a b] = c;", &format!("{ARRAY}, found identifier"), "1:4"),
        (
            "for ([a b] of c);",
            &format!("{ARRAY}, found identifier"),
            "1:9",
        ),
    ]);
}

/// The siblings whose wording the array follows.
#[test]
fn control_sibling_lists() {
    check(&[
        (
            "({a: 1 b: 2})",
            "Expected ',' or '}' after list element, found identifier",
            "1:8",
        ),
        (
            "f(1 2)",
            "Expected ',' or ')' after list element, found number",
            "1:5",
        ),
        (
            "function f(a b) {}",
            "Expected ',' or ')' after list element, found identifier",
            "1:14",
        ),
        // an element that does not start is still an expression the parser wanted
        ("[1, ;]", "Expected expression, found ';'", "1:5"),
    ]);
}
