// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! `export async` must be followed by `function` on the same line, and when it is not, the
//! error names what IS there — the token after `async`, at that token — the way its
//! `export declare async` sibling does.
//!
//! acorn rejects both spellings at the `async` (`Unexpected token (1:7)`); the message is
//! tsv's own, and a `found 'async'` reads as though the keyword itself were the stray token.
//! The line-break spelling (`async [no LineTerminator here] function`, ecma262) is a
//! different fault with a token that IS `function`, so it takes the sibling's own message.
//!
//! Not fixturable: `input_invalid_*` asserts only that both parsers reject, never the
//! message or its point.

/// The rendered error for `source`, parsed as a module: its message line and its
/// `line:col` header.
fn error(source: &str) -> (String, String) {
    let arena = bumpalo::Bump::new();
    let rendered = tsv_ts::parse_with_goal(source, tsv_ts::Goal::Module, &arena)
        .expect_err("the export must not parse")
        .to_string();
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

#[test]
fn export_async_names_the_token_after_async() {
    check(&[
        (
            "export async x",
            "Expected 'function' after 'export async', found identifier",
            "1:14",
        ),
        (
            "export async class C {}",
            "Expected 'function' after 'export async', found 'class'",
            "1:14",
        ),
        (
            "export async",
            "Expected 'function' after 'export async', found end of file",
            "1:13",
        ),
    ]);
}

#[test]
fn export_async_across_a_line_break_names_the_break() {
    check(&[(
        "export async\nfunction f() {}",
        "'function' must be on the same line as 'async'",
        "2:1",
    )]);
}

/// The sibling whose wording the `export async` errors follow.
#[test]
fn control_export_declare_async() {
    check(&[
        (
            "export declare async x",
            "Expected 'function' after 'declare async', found identifier",
            "1:22",
        ),
        (
            "export declare async\nfunction f()",
            "'function' must be on the same line as 'async'",
            "2:1",
        ),
    ]);
}
