// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The width boundary of an `@scope` clause where the text behind it is not the plain
//! `) {` the fixture measures: each clause is fitted on its line through whatever follows
//! it up to the next place a break can land, and two shapes move that point.
//!
//! - **A multi-line comment before the `{`.** The comment's first line ends the clause's
//!   line and carries the block opener down with it, so the ` {` is not on that line and
//!   does not count against it.
//! - **Empty limit parens (`to ()`).** They hold nothing to drop, so the root's line runs
//!   on through `) to () {` and the root is the clause that opens.
//!
//! Pinned here beside `css/at_rules/scope_long_prettier_divergence`, which holds the plain
//! `) {` and `) to (` boundaries: these are the same rule read through a different tail.

fn format_css(source: &str) -> String {
    tsv_css::format_str(source).expect("format failed")
}

/// The first line's width — every line here is ASCII with no tab.
fn head_width(source: &str) -> usize {
    source.lines().next().expect("a first line").len()
}

/// `source` is `width` columns on its first line, is a fixed point when `expected` is
/// `None`, and otherwise formats to `expected`, itself a fixed point.
fn assert_boundary(label: &str, source: &str, width: usize, expected: Option<&str>) {
    assert_eq!(head_width(source), width, "case `{label}`: authored width");
    let expected = expected.unwrap_or(source);
    let formatted = format_css(source);
    assert_eq!(formatted, expected, "case `{label}`");
    assert_eq!(
        format_css(&formatted),
        formatted,
        "case `{label}`: the output must be a fixed point"
    );
}

/// `@scope (.aaa…)<rest>` with the root sized so the head line is `width` columns, where
/// `rest_head` is the part of `rest` on that line.
fn rule(width: usize, rest_head: &str, rest: &str) -> (String, String) {
    let root = "a".repeat(width - "@scope (.".len() - rest_head.len());
    (format!("@scope (.{root}{rest}"), root)
}

#[test]
fn a_multiline_comment_before_the_block_takes_the_opener_off_the_limit_line() {
    let rest = ") to (.l) /* a\n b */ {\n}\n";
    let (source, _) = rule(100, ") to (.l) /* a", rest);
    assert_boundary("limit, 100 columns", &source, 100, None);

    let (source, root) = rule(101, ") to (.l) /* a", rest);
    let expected = format!("@scope (.{root}) to (\n\t.l\n) /* a\n b */ {{\n}}\n");
    assert_boundary("limit, 101 columns", &source, 101, Some(&expected));
}

#[test]
fn a_multiline_comment_before_the_block_takes_the_opener_off_the_root_line() {
    let rest = ") /* a\n b */ {\n}\n";
    let (source, _) = rule(100, ") /* a", rest);
    assert_boundary("root only, 100 columns", &source, 100, None);

    let (source, root) = rule(101, ") /* a", rest);
    let expected = format!("@scope (\n\t.{root}\n) /* a\n b */ {{\n}}\n");
    assert_boundary("root only, 101 columns", &source, 101, Some(&expected));
}

#[test]
fn an_empty_limit_is_measured_on_the_root_line() {
    let rest = ") to () {\n}\n";
    let (source, _) = rule(100, ") to () {", rest);
    assert_boundary("empty limit, 100 columns", &source, 100, None);

    let (source, root) = rule(101, ") to () {", rest);
    let expected = format!("@scope (\n\t.{root}\n) to () {{\n}}\n");
    assert_boundary("empty limit, 101 columns", &source, 101, Some(&expected));
}
