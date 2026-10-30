// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A method signature keyed by the value `new` keeps its quotes, and only that key does.
//!
//! The fixture pins the rule inside a Svelte `<script>`
//! ([method_signature_new_key_quoted](../tests/fixtures/typescript/types/method_signature_new_key_quoted/)).
//! This file holds the rule's edges the fixture does not spell: a computed `['new']` key,
//! which keeps its brackets, the near spellings of `new` that are other names and unquote,
//! the escaped spellings of the value `new`, which keep their quotes and escapes, and the
//! standalone-file entry point.

/// Format a standalone TypeScript source, asserting the output is a fixed point.
fn format_ts(source: &str) -> String {
    let once = tsv_ts::format(source).expect("format failed");
    let twice = tsv_ts::format(&once).expect("the output failed to format");
    assert_eq!(once, twice, "not a fixed point: {source:?}");
    once
}

/// A standalone file keeps the quotes in an interface and a type literal, plain, generic,
/// optional and accessor alike; the quote style normalizes.
#[test]
fn standalone_file_keeps_new_method_signature_quotes() {
    let source = "interface I {\n\t\"new\"(): void;\n\t'new'<T>(a: T): T;\n\t'new'?(): void;\n}\ntype T = { get 'new'(): number; set \"new\"(v: number) };\n";
    let expected = "interface I {\n\t'new'(): void;\n\t'new'<T>(a: T): T;\n\t'new'?(): void;\n}\ntype T = { get 'new'(): number; set 'new'(v: number) };\n";
    assert_eq!(format_ts(source), expected);
}

/// A computed key is printed in its brackets, never as the bare quoted literal.
#[test]
fn computed_new_key_keeps_its_brackets() {
    let source = "interface I {\n\t['new'](): void;\n\t[\"new\"]?(): void;\n}\n";
    let expected = "interface I {\n\t['new'](): void;\n\t['new']?(): void;\n}\n";
    assert_eq!(format_ts(source), expected);
}

/// A key that only resembles `new` is another name, and unquotes like any other.
#[test]
fn near_spellings_of_new_unquote() {
    let source = "interface I {\n\t'New'(): void;\n\t'NEW'(): void;\n\t'renew'(): void;\n\t'new_'(): void;\n}\n";
    let expected =
        "interface I {\n\tNew(): void;\n\tNEW(): void;\n\trenew(): void;\n\tnew_(): void;\n}\n";
    assert_eq!(format_ts(source), expected);
}

/// An escaped spelling of the value `new` keeps its quotes and its escapes as written.
#[test]
fn escaped_new_keys_stay_as_written() {
    let source =
        "interface I {\n\t'n\\x65w'(): void;\n\t'\\u{6e}ew'(): void;\n\t'ne\\u0077'(): void;\n}\n";
    assert_eq!(format_ts(source), source);
}
