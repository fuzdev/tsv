// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A type-argument list binds to the type before it only when no line terminator
//! intervenes — tsc guards `parseTypeArgumentsOfTypeReference` with
//! `!scanner.hasPrecedingLineBreak()`. A `TSImportType`'s qualifier is one of the sites
//! that rule covers, so `import('./a').B⏎<T>` is the type `import('./a').B` followed by
//! a separate `<T>`, not `import('./a').B<T>`. acorn-typescript welds across the break,
//! the same recovery it performs for a plain type reference.
//!
//! The non-member rejection is pinned as a fixture
//! (`types/type_args/import_type_line_break_svelte_divergence`, where
//! `expected_svelte.json` proves acorn still accepts). Two things can't live there. One
//! fixture carries one `tsv_rejects.txt` substring, so the remaining spellings — no
//! qualifier, and the `typeof import(…)` composition, which `parse_type_query`'s own
//! guard runs too late to see because the import body has already consumed the `<` —
//! are pinned here. And the **type-member** direction, where the split is legal and
//! yields two members, has no fixture at all: acorn *rejects* it (having welded the type
//! arguments, it finds `(): C` with no separator), so there is no `expected.json` oracle
//! even though tsc agrees with tsv.
//!
//! That member case is the load-bearing one. Every rejection below surfaces as the same
//! downstream `Expected expression` — the guard's effect is a *narrower type*, not an
//! error of its own — so only a shape assertion proves the type really ended at the
//! qualifier rather than something else failing.
//!
//! `tuple_optional_marker_line_break.rs` is the sibling for the postfix-`?` rule.

use serde_json::Value;

const LEFTOVER_ERROR: &str = "Expected expression, found ';'";

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source)
}

/// Assert tsv rejects `source` because the type ended before the `<`, leaving the
/// type-argument list stranded as an expression.
#[track_caller]
fn assert_stranded_type_arguments(source: &str) {
    let arena = bumpalo::Bump::new();
    let error = tsv_ts::parse(source, &arena)
        .err()
        .map_or_else(|| "<parsed successfully>".to_owned(), |e| e.to_string());
    assert!(
        error.contains(LEFTOVER_ERROR),
        "expected the stranded type-argument rejection for {source:?}, got: {error}"
    );
}

/// A raw newline after the qualifier — the base form, also the fixture's input.
#[test]
fn qualifier_line_break_rejected() {
    assert_stranded_type_arguments("type A = import('./a').B\n<string>;");
}

/// No qualifier at all: the guard sits after the optional `.Foo`, so the bare
/// `import('./a')` head takes it too.
#[test]
fn bare_import_line_break_rejected() {
    assert_stranded_type_arguments("type A = import('./a')\n<string>;");
}

/// `typeof import(…)` composes through the same body. `parse_type_query` has its own
/// same-line guard, but it runs *after* the import body would already have consumed the
/// `<`, so this spelling is covered by the body's gate rather than by the query's.
#[test]
fn typeof_import_line_break_rejected() {
    assert_stranded_type_arguments("type A = typeof import('./a')\n<string>;");
}

/// …and with a qualifier, the composition of both optional parts.
#[test]
fn typeof_import_qualifier_line_break_rejected() {
    assert_stranded_type_arguments("type A = typeof import('./a').B\n<string>;");
}

/// Per ecma262 §sec-comments a `MultiLineComment` containing a line terminator *is* a
/// `LineTerminator`, so a multi-line block comment triggers the rule as well.
#[test]
fn multiline_block_comment_before_type_args_rejected() {
    assert_stranded_type_arguments("type A = import('./a').B /* c\nd */ <string>;");
}

/// The member direction, and the only assertion that proves *where* the type ended: the
/// break splits one member into two, the property's type keeping no type arguments.
#[test]
fn type_member_line_break_splits_into_two_members() {
    let json = parse_json("type I = { a: import('./a').B\n<T>(): C };");
    let members = "/body/0/typeAnnotation/members";
    assert_eq!(
        json.pointer(members)
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "the break ends the property member and starts a call signature: {json}"
    );
    let property_type = format!("{members}/0/typeAnnotation/typeAnnotation");
    assert_eq!(
        json.pointer(&format!("{property_type}/type"))
            .and_then(Value::as_str),
        Some("TSImportType"),
        "the first member's type is the import type: {json}"
    );
    assert!(
        json.pointer(&format!("{property_type}/typeArguments"))
            .is_none(),
        "and it carries no type arguments: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{members}/1/type"))
            .and_then(Value::as_str),
        Some("TSCallSignatureDeclaration"),
        "the stranded `<T>(): C` is its own member: {json}"
    );
}

/// Control: on one line the arguments bind, so the import type keeps them.
#[test]
fn same_line_type_arguments_bind() {
    let json = parse_json("type A = import('./a').B<string>;");
    let import_type = "/body/0/typeAnnotation";
    assert_eq!(
        json.pointer(&format!("{import_type}/type"))
            .and_then(Value::as_str),
        Some("TSImportType"),
        "the annotation is the import type: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{import_type}/typeArguments/params/0/type"))
            .and_then(Value::as_str),
        Some("TSStringKeyword"),
        "and it carries `<string>`: {json}"
    );
}

/// Control: a same-line block comment in the gap is trivia, not a terminator, so the
/// arguments still bind — the discrimination is the line break, not the gap's contents.
#[test]
fn same_line_block_comment_keeps_type_arguments_bound() {
    let json = parse_json("type A = import('./a').B /* c */ <string>;");
    assert_eq!(
        json.pointer("/body/0/typeAnnotation/typeArguments/params/0/type")
            .and_then(Value::as_str),
        Some("TSStringKeyword"),
        "a same-line comment keeps the arguments bound: {json}"
    );
}
