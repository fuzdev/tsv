// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Wire-level decoding of `\u` unicode escapes in identifier NAME positions that
//! the fixture pipeline can only exercise through the formatter. A fixture's
//! `expected.json` is generated from its prettier-formatted `input.*` (the
//! decoded form), so the escaped source — the actual repro — only ever lives in
//! an `unformatted_*` variant, which validates FORMATTING normalization, not the
//! emitted AST. These tests pin the AST `name` (and, for the constructor,
//! `kind`) that acorn decodes per ecma262 IdentifierName StringValue: an
//! `IdentifierName`'s value is its code points with every `UnicodeEscapeSequence`
//! resolved, so `x.\u0061` names property `a`, not `\u0061`. Var-bindings and
//! object-literal keys already decoded; these cover the property/member and
//! class/interface member-key positions that were span-identity-only.
//!
//! The escaped-`constructor` case additionally can't be a fixture at all:
//! prettier's babel-ts parser rejects `\u0063onstructor` ("Keywords cannot
//! contain escape characters") though acorn-ts (tsv's parse oracle) accepts it
//! as the constructor — so no prettier oracle exists to format against.

use serde_json::Value;

/// Parses `source`, asserts the wire `name` at `name_pointer` decodes to
/// `expected_name`, then asserts tsv formats `source` to `expected_output` (the
/// escape decoded) and that the output is a stable fixed point whose reparse
/// carries the same decoded name.
fn assert_name_decodes(
    source: &str,
    name_pointer: &str,
    expected_name: &str,
    expected_output: &str,
) {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_ts::Interner::new();
    let program = tsv_ts::parse(source, &arena, &mut interner).expect("parse failed");
    let json = tsv_ts::convert_ast_json(&program, source, &interner);

    assert_eq!(
        json.pointer(name_pointer).and_then(Value::as_str),
        Some(expected_name),
        "wire name decodes (acorn IdentifierName StringValue): {json}"
    );

    let output = tsv_ts::format(&program, source, &interner);
    assert_eq!(
        output, expected_output,
        "formatter decodes the escaped name"
    );

    // The decoded output is tsv's fixed point and reparses to the same name.
    let arena_out = bumpalo::Bump::new();
    let mut interner_out = tsv_ts::Interner::new();
    let reparsed = tsv_ts::parse(&output, &arena_out, &mut interner_out).expect("reparse failed");
    assert_eq!(
        tsv_ts::format(&reparsed, &output, &interner_out),
        output,
        "output should be stable"
    );
    let json_out = tsv_ts::convert_ast_json(&reparsed, &output, &interner_out);
    assert_eq!(
        json_out.pointer(name_pointer).and_then(Value::as_str),
        Some(expected_name),
        "reparsed name matches"
    );
}

/// Member-expression property: `x.\u0061` → property named `a`.
#[test]
fn member_expression_property() {
    assert_name_decodes(
        r"const y = x.\u0061;",
        "/body/0/declarations/0/init/property/name",
        "a",
        "const y = x.a;\n",
    );
}

/// `new`-expression member callee: `new X.\u0061()` → property named `a`.
#[test]
fn new_expression_member_property() {
    assert_name_decodes(
        r"const y = new X.\u0061();",
        "/body/0/declarations/0/init/callee/property/name",
        "a",
        "const y = new X.a();\n",
    );
}

/// Class member key (here an escaped keyword): `\u0069n` → key named `in`.
#[test]
fn class_member_key() {
    assert_name_decodes(
        r"class C { \u0069n: string; }",
        "/body/0/body/body/0/key/name",
        "in",
        "class C {\n\tin: string;\n}\n",
    );
}

/// Interface / type-literal member key (shared `parse_type_members` path):
/// `\u0061` → key named `a`.
#[test]
fn interface_member_key() {
    assert_name_decodes(
        r"interface I { \u0061: string; }",
        "/body/0/body/body/0/key/name",
        "a",
        "interface I {\n\ta: string;\n}\n",
    );
}

/// An escaped `constructor` key IS the class constructor (`kind: "constructor"`),
/// matched by decoded StringValue — acorn parity. Prettier's babel-ts parser
/// rejects the escaped form, so this can only be a root test.
#[test]
fn escaped_constructor_is_constructor() {
    let source = r"class C { \u0063onstructor() {} }";
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_ts::Interner::new();
    let program = tsv_ts::parse(source, &arena, &mut interner).expect("parse failed");
    let json = tsv_ts::convert_ast_json(&program, source, &interner);
    let member = json.pointer("/body/0/body/body/0").expect("class member");
    assert_eq!(
        member.pointer("/kind").and_then(Value::as_str),
        Some("constructor"),
        "escaped constructor detected by decoded StringValue: {member}"
    );
    assert_eq!(
        member.pointer("/key/name").and_then(Value::as_str),
        Some("constructor"),
        "constructor key name decodes: {member}"
    );
    assert_eq!(
        tsv_ts::format(&program, source, &interner),
        "class C {\n\tconstructor() {}\n}\n",
        "formatter decodes the escaped constructor"
    );
}
