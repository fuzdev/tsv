// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The postfix optional `?` of a tuple element is a `[no LineTerminator here]`
//! position: tsc runs its whole postfix suffix loop — `?`, `!` and `[` alike — under
//! `while (!scanner.hasPrecedingLineBreak())` (`parsePostfixTypeOrHigher`), so `[T⏎?]`
//! is not an optional element and the stray `?` is a syntax error. oxc agrees;
//! acorn-typescript and babel accept the break, spelling the guard for the array
//! suffix one function below and omitting it here.
//!
//! The raw-break form is pinned as a fixture
//! (`types/tuple_optional_marker_line_break_svelte_divergence`, where
//! `expected_svelte.json` proves acorn still accepts). What can't live there is the rest
//! of the matrix: one fixture carries one `tsv_rejects.txt` substring, and the ACCEPT
//! rows have no divergence to record. So the comment-borne triggers, the same-line
//! control, and the named-member marker that legitimately takes the break are pinned
//! here, against tsv's own verdict.
//!
//! `asi_postfix_bracket_type.rs` is the array-suffix sibling of the same rule.

use serde_json::Value;

const LINE_BREAK_ERROR: &str = "Optional tuple element `?` cannot follow a line terminator";

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source)
}

/// Assert tsv rejects `source` on the `[no LineTerminator here]` rule specifically — a
/// rejection with any other message would mean the guard fired for the wrong reason.
#[track_caller]
fn assert_line_break_rejected(source: &str) {
    let arena = bumpalo::Bump::new();
    let error = tsv_ts::parse(source, &arena)
        .err()
        .map_or_else(|| "<parsed successfully>".to_owned(), |e| e.to_string());
    assert!(
        error.contains(LINE_BREAK_ERROR),
        "expected the line-terminator rejection for {source:?}, got: {error}"
    );
}

/// A raw newline before the `?` — the base form, also the fixture's input.
#[test]
fn raw_line_break_before_marker_rejected() {
    assert_line_break_rejected("type A = [T\n?];");
}

/// A `//` comment runs to end-of-line, so it always carries the terminator with it.
#[test]
fn line_comment_before_marker_rejected() {
    assert_line_break_rejected("type A = [T // c\n?];");
}

/// Per ecma262 §sec-comments a `MultiLineComment` containing a line terminator *is* a
/// `LineTerminator`, so a multi-line block comment triggers the rule as well — the
/// authoring that looks most like it should survive.
#[test]
fn multiline_block_comment_before_marker_rejected() {
    assert_line_break_rejected("type A = [T /* c\nd */?];");
}

/// The non-identifier operand path (`parse_type` then the postfix `?`) takes the same
/// guard as the identifier lookahead path, so a parenthesized element rejects too.
#[test]
fn parenthesized_element_line_break_rejected() {
    assert_line_break_rejected("type A = [(T | U)\n?];");
}

/// Control: a same-line block comment in the gap is NOT a terminator, so the element
/// stays optional (its formatting is the `types/tuple_optional_comment` fixture).
#[test]
fn same_line_block_comment_stays_optional() {
    let json = parse_json("type A = [T /* c */?];");
    assert_eq!(
        json.pointer("/body/0/typeAnnotation/elementTypes/0/type")
            .and_then(Value::as_str),
        Some("TSOptionalType"),
        "same-line comment keeps the optional element: {json}"
    );
}

/// Control: the **named**-member `?:` marker is a different grammar position (tsc reads
/// it through `parseOptionalToken`, outside the postfix loop) and does take the break.
#[test]
fn named_member_marker_takes_the_line_break() {
    let json = parse_json("type A = [a\n?: T];");
    let member = "/body/0/typeAnnotation/elementTypes/0";
    assert_eq!(
        json.pointer(&format!("{member}/type"))
            .and_then(Value::as_str),
        Some("TSNamedTupleMember"),
        "a break before the named marker is allowed: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{member}/optional"))
            .and_then(Value::as_bool),
        Some(true),
        "and the member is still optional: {json}"
    );
}
