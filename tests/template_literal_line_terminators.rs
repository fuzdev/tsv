// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Wire-level `<CR>` normalization in a template element's `raw` and `cooked`.
//! ECMAScript defines both the TRV and the TV of a `LineTerminatorSequence` in
//! a template: `<CR><LF>` and a lone `<CR>` each map to a single `<LF>`, while
//! `<LF>`, `<LS>` and `<PS>` map to themselves (`../ecma262`, the TRV/TV
//! productions for `TemplateCharacter`). So a template element's `raw` is not a
//! source slice whenever the source carries a `<CR>` — `` `x<CR><LF>y` `` has a
//! 4-byte source span and a 3-character `raw`.
//!
//! These can't be fixtures: prettier rewrites every `<CR>` in its output (an
//! `input.*` carrying one is not a prettier fixed point, so F1 fails), and the
//! `unformatted_*` variant that could hold the bytes makes a FORMATTING claim,
//! never an AST one. Each test therefore also asserts the premise — tsv formats
//! the `<CR>` source to itself — so if tsv ever adopts prettier's output-side
//! normalization these cases flag for promotion into a real fixture.
//!
//! Two sets of null controls, varying the same dimension in opposite ways:
//! `<LF>`/`<LS>`/`<PS>` are line terminators the TRV deliberately does not
//! rewrite, and an escaped `\r` is the character U+000D arriving by a route that
//! is not a `LineTerminatorSequence` at all. The second set is the one that
//! bites — after decoding the two are the same character, so a normalization
//! applied to the cooked value instead of the raw source silently rewrites real
//! code (an HTTP request fixture, caught by `corpus:compare:parse`).

use serde_json::Value;

/// Parses `source`, asserts the template element at `quasi_pointer` carries
/// `expected_raw` / `expected_cooked` (`None` = the wire's `null`), then
/// asserts tsv formats `source` to itself — the premise that keeps this out of
/// the fixture pipeline.
fn assert_quasi(
    source: &str,
    quasi_pointer: &str,
    expected_raw: &str,
    expected_cooked: Option<&str>,
) {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    let json = tsv_ts::convert_ast_json(&program, source);
    let quasi = json.pointer(quasi_pointer).expect("template element");

    assert_eq!(
        quasi.pointer("/value/raw").and_then(Value::as_str),
        Some(expected_raw),
        "TRV of the element's LineTerminatorSequences: {quasi}"
    );
    assert_eq!(
        quasi.pointer("/value/cooked").and_then(Value::as_str),
        expected_cooked,
        "TV of the element's LineTerminatorSequences: {quasi}"
    );

    assert_eq!(
        tsv_ts::format(&program, source),
        source,
        "premise: tsv keeps the source's line terminators, so prettier — which \
         rewrites them — is what blocks a fixture. If this fails, promote these \
         cases to a fixture."
    );
}

/// `<CR><LF>` in an untagged template: one `<LF>` in both `raw` and `cooked`.
#[test]
fn crlf_normalizes_to_lf() {
    assert_quasi(
        "const a = `x\r\ny`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "x\ny",
        Some("x\ny"),
    );
}

/// A lone `<CR>`: also one `<LF>`, though it spans a single source byte.
#[test]
fn lone_cr_normalizes_to_lf() {
    assert_quasi(
        "const a = `p\rq`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "p\nq",
        Some("p\nq"),
    );
}

/// `<LF>` is already the normalized form — the identity control.
#[test]
fn lf_is_unchanged() {
    assert_quasi(
        "const a = `x\ny`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "x\ny",
        Some("x\ny"),
    );
}

/// `<LS>` (U+2028) is a LineTerminatorSequence the TRV maps to itself.
#[test]
fn ls_is_unchanged() {
    assert_quasi(
        "const a = `x\u{2028}y`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "x\u{2028}y",
        Some("x\u{2028}y"),
    );
}

/// `<PS>` (U+2029), the sibling control.
#[test]
fn ps_is_unchanged() {
    assert_quasi(
        "const a = `x\u{2029}y`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "x\u{2029}y",
        Some("x\u{2029}y"),
    );
}

/// A tagged template — the position where `raw` is observable at runtime
/// (`String.raw`), so the normalization is a value, not just a wire field.
#[test]
fn tagged_template_crlf_normalizes() {
    assert_quasi(
        "fn`x\r\ny`;\n",
        "/body/0/expression/quasi/quasis/0",
        "x\ny",
        Some("x\ny"),
    );
}

/// A line continuation across `<CR><LF>`: the TRV keeps the `\` and normalizes
/// the terminator behind it, while the TV drops the whole continuation.
#[test]
fn line_continuation_crlf_normalizes_raw_only() {
    assert_quasi(
        "const a = `x\\\r\ny`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "x\\\ny",
        Some("xy"),
    );
}

/// An escape BESIDE the `<CR>` routes the cooked value through the lexer's
/// decoder instead of the verbatim slice — a second code path, and the decoder
/// copies a raw terminator through untouched (only escapes are its business).
#[test]
fn decoded_cooked_normalizes_too() {
    assert_quasi(
        "const a = `\\tx\r\ny`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "\\tx\ny",
        Some("\tx\ny"),
    );
}

/// The same over a lone `<CR>`, where the decoded value's `<CR>` is the only
/// line terminator present.
#[test]
fn decoded_cooked_normalizes_lone_cr() {
    assert_quasi(
        "const a = `\\u0041x\ry`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "\\u0041x\ny",
        Some("Ax\ny"),
    );
}

/// ⚠️ A `\r` the author wrote as an ESCAPE is not a `LineTerminatorSequence` and
/// must survive — the control that separates "a literal terminator in the
/// template body" from "the character U+000D". They are indistinguishable once
/// decoded, so normalizing a cooked value instead of the raw source rewrites
/// this, and HTTP fixtures in real code are full of it.
#[test]
fn escaped_cr_is_not_a_line_terminator() {
    assert_quasi(
        "const a = `GET / HTTP/1.1\\r\\nHost: x\\r\\n\\r\\n`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "GET / HTTP/1.1\\r\\nHost: x\\r\\n\\r\\n",
        Some("GET / HTTP/1.1\r\nHost: x\r\n\r\n"),
    );
}

/// The two together, in one element: the literal `<CR><LF>` normalizes to a
/// `<LF>` while the escaped `\r` beside it decodes to U+000D and stays.
#[test]
fn literal_and_escaped_cr_in_one_element() {
    assert_quasi(
        "const a = `x\\r\r\ny`;\n",
        "/body/0/declarations/0/init/quasis/0",
        "x\\r\ny",
        Some("x\r\ny"),
    );
}

/// An invalid escape in a tagged template: `cooked` is `null`, and `raw` still
/// normalizes.
#[test]
fn invalid_escape_normalizes_raw_with_null_cooked() {
    assert_quasi(
        "fn`x\\u{}\r\ny`;\n",
        "/body/0/expression/quasi/quasis/0",
        "x\\u{}\ny",
        None,
    );
}

/// A template literal TYPE. `parse_template_literal_type` is a parallel
/// implementation of the expression parser ("kept separate for clarity despite
/// duplication"), so it is the site most likely to drift back — it shares the
/// element constructor precisely so it cannot. The source is spelled with the
/// break after `=` that a multiline template RHS takes, so the premise
/// assertion still measures the terminators rather than the layout.
#[test]
fn template_literal_type_crlf_normalizes() {
    assert_quasi(
        "type T =\n\t`x\r\ny`;\n",
        "/body/0/typeAnnotation/literal/quasis/0",
        "x\ny",
        Some("x\ny"),
    );
}

/// A `<CR><LF>` in each of a multi-element template's quasis — the middle and
/// tail elements take a different parse path than the head.
#[test]
fn every_quasi_normalizes() {
    let source = "fn`a\r\n${x}b\r\n${y}c\r\n`;\n";
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    let json = tsv_ts::convert_ast_json(&program, source);

    for (i, expected) in ["a\n", "b\n", "c\n"].iter().enumerate() {
        let quasi = json
            .pointer(&format!("/body/0/expression/quasi/quasis/{i}"))
            .expect("template element");
        assert_eq!(
            quasi.pointer("/value/raw").and_then(Value::as_str),
            Some(*expected),
            "quasi {i} raw: {quasi}"
        );
        assert_eq!(
            quasi.pointer("/value/cooked").and_then(Value::as_str),
            Some(*expected),
            "quasi {i} cooked: {quasi}"
        );
    }
}
