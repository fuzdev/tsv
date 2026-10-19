// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A quoted enum member name prints under the as-needed key rule every other key
//! follows: an identifier-valid name drops its quotes, and a name carrying an escape or
//! not spelling an identifier keeps them.
//!
//! The fixture pins the rule inside a Svelte `<script>`
//! ([member_key_unquote](../tests/fixtures/typescript/declarations/enum/member_key_unquote/)).
//! This file holds the `export declare` forms, which no fixture can carry while their
//! wire field order differs from acorn's.

/// Format a standalone TypeScript source, asserting the output is a fixed point.
fn format_ts(source: &str) -> String {
    let once = tsv_ts::format_str(source).expect("format failed");
    let twice = tsv_ts::format_str(&once).expect("the output failed to format");
    assert_eq!(once, twice, "not a fixed point: {source:?}");
    once
}

/// The `export declare` forms unquote like every other enum declaration.
// TODO: move these cells into the `member_key_unquote` fixture once the `export declare`
// enum forms' wire field order matches acorn's (`declare` after `members`), then delete
// this file.
#[test]
fn export_declare_enum_unquotes_member_names() {
    let source = concat!(
        "export declare enum E {\n\t'a' = 1,\n\t\"b\"\n}\n",
        "export declare const enum F {\n\t'a' = 1\n}\n"
    );
    let expected =
        "export declare enum E {\n\ta = 1,\n\tb\n}\nexport declare const enum F {\n\ta = 1\n}\n";
    assert_eq!(format_ts(source), expected);
}
