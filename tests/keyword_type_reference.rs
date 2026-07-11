// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! TypeScript's type space is a separate namespace where a `TypeName` is an
//! `IdentifierName` — reserved *statement* keywords (`break`, `default`,
//! `function`, `case`, `switch`, `while`, …) are valid type-reference names there.
//! tsc and prettier both accept `let x: break;` (prettier formats it), so tsv must
//! parse it as a `TSTypeReference` whose `typeName` is a plain `Identifier`.
//!
//! This can't be a fixture: acorn-typescript is over-strict in type position and
//! *rejects* these ("Expected type"), so no `expected.json` oracle exists — the
//! shape is pinned here against tsv itself and prettier's formatting.
//!
//! Contrast: the keywords that head their own type production stay on that
//! production and do NOT collapse to a bare type reference — `void`/`null`/`string`
//! are primitive `TSKeyword*` types, `this` is a `TSThisType`, `typeof x` a
//! `TSTypeQuery`, `new () => T` a `TSConstructorType`, `import('m')` a
//! `TSImportType` (regression guards below).

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source)
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

fn rejects(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_err()
}

/// Reserved keywords prettier accepts as a bare type-reference name (each verified
/// with `format_prettier --content 'let x: <kw>;'`). `static`/`yield`/`await`/`let`
/// are included even though some lex as contextual identifiers — the end-state is
/// the same `TSTypeReference` regardless of how the head token lexes.
const KEYWORD_TYPE_NAMES: &[&str] = &[
    "break",
    "default",
    "static",
    "function",
    "case",
    "yield",
    "await",
    "while",
    "do",
    "continue",
    "debugger",
    "delete",
    "in",
    "instanceof",
    "switch",
    "try",
    "catch",
    "finally",
    "throw",
    "extends",
    "enum",
    "class",
    "else",
    "for",
    "if",
    "return",
    "var",
    "let",
    "super",
    "export",
];

/// Each reserved keyword parses as an annotation that is a `TSTypeReference` whose
/// `typeName` is a plain `Identifier` carrying the keyword text.
#[test]
fn reserved_keywords_parse_as_type_reference() {
    let ann = "/body/0/declarations/0/id/typeAnnotation/typeAnnotation";
    for kw in KEYWORD_TYPE_NAMES {
        let src = format!("let x: {kw};");
        let json = parse_json(&src);
        assert_eq!(
            json.pointer(&format!("{ann}/type")).and_then(Value::as_str),
            Some("TSTypeReference"),
            "`{src}` annotation is a TSTypeReference: {json}"
        );
        assert_eq!(
            json.pointer(&format!("{ann}/typeName/type"))
                .and_then(Value::as_str),
            Some("Identifier"),
            "`{src}` typeName is an Identifier: {json}"
        );
        assert_eq!(
            json.pointer(&format!("{ann}/typeName/name"))
                .and_then(Value::as_str),
            Some(*kw),
            "`{src}` typeName.name is the keyword: {json}"
        );
    }
}

/// A keyword head qualifies: `break.foo` is a `TSTypeReference` whose `typeName` is
/// a `TSQualifiedName` (the entity-name loop reads the `.foo` right side).
#[test]
fn keyword_qualified_type_name_parses() {
    let json = parse_json("let x: break.foo;");
    let ann = "/body/0/declarations/0/id/typeAnnotation/typeAnnotation";
    assert_eq!(
        json.pointer(&format!("{ann}/type")).and_then(Value::as_str),
        Some("TSTypeReference"),
    );
    assert_eq!(
        json.pointer(&format!("{ann}/typeName/type"))
            .and_then(Value::as_str),
        Some("TSQualifiedName"),
        "a keyword head qualifies over `.`: {json}"
    );
}

/// A keyword head takes type arguments: `yield<T>` is a `TSTypeReference` with a
/// `typeArguments` instantiation (the shared optional-type-arguments guard runs).
#[test]
fn keyword_generic_type_reference_parses() {
    let json = parse_json("let x: yield<T>;");
    let ann = "/body/0/declarations/0/id/typeAnnotation/typeAnnotation";
    assert_eq!(
        json.pointer(&format!("{ann}/type")).and_then(Value::as_str),
        Some("TSTypeReference"),
    );
    assert_eq!(
        json.pointer(&format!("{ann}/typeArguments/type"))
            .and_then(Value::as_str),
        Some("TSTypeParameterInstantiation"),
        "the `<T>` is parsed as type arguments: {json}"
    );
}

/// A keyword type reference stands as a union member: `type U = A | default;` is a
/// `TSUnionType` whose second member is the keyword type reference.
#[test]
fn keyword_union_member_parses() {
    let json = parse_json("type U = A | default;");
    let ty = "/body/0/typeAnnotation";
    assert_eq!(
        json.pointer(&format!("{ty}/type")).and_then(Value::as_str),
        Some("TSUnionType"),
    );
    assert_eq!(
        json.pointer(&format!("{ty}/types/1/type"))
            .and_then(Value::as_str),
        Some("TSTypeReference"),
        "the `default` member is a type reference: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{ty}/types/1/typeName/name"))
            .and_then(Value::as_str),
        Some("default"),
    );
}

/// A few keyword annotations format to themselves (idempotent — the keyword text
/// survives the round-trip, matching prettier).
#[test]
fn keyword_type_references_format_to_themselves() {
    for canonical in [
        "let x: break;\n",
        "let x: default;\n",
        "let x: break.foo;\n",
        "type U = A | default;\n",
    ] {
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

// -- Regression guards: keywords with their own type production must NOT collapse
//    to a bare type reference. --

/// `string` stays the primitive `TSStringKeyword` (the `from_lexer_keyword` fast
/// path wins over the new keyword arm).
#[test]
fn primitive_keyword_stays_primitive() {
    let ann = "/body/0/declarations/0/id/typeAnnotation/typeAnnotation";
    for (src, expected) in [
        ("let x: string;", "TSStringKeyword"),
        ("let x: void;", "TSVoidKeyword"),
        ("let x: null;", "TSNullKeyword"),
    ] {
        let json = parse_json(src);
        assert_eq!(
            json.pointer(&format!("{ann}/type")).and_then(Value::as_str),
            Some(expected),
            "`{src}` stays {expected}: {json}"
        );
    }
}

/// `this` in type position stays a `TSThisType`, not a type reference named "this".
#[test]
fn this_type_unaffected() {
    let json = parse_json("let x: this;");
    assert_eq!(
        json.pointer("/body/0/declarations/0/id/typeAnnotation/typeAnnotation/type")
            .and_then(Value::as_str),
        Some("TSThisType"),
        "`this` stays a TSThisType: {json}"
    );
}

/// `new () => void` stays a `TSConstructorType` (the `New` keyword arm still wins).
#[test]
fn constructor_type_unaffected() {
    let json = parse_json("type C = new () => void;");
    assert_eq!(
        json.pointer("/body/0/typeAnnotation/type")
            .and_then(Value::as_str),
        Some("TSConstructorType"),
        "`new () => void` stays a TSConstructorType: {json}"
    );
}

/// `typeof x` stays a `TSTypeQuery` (the `Typeof` keyword arm still wins).
#[test]
fn type_query_unaffected() {
    let json = parse_json("let y: typeof x;");
    assert_eq!(
        json.pointer("/body/0/declarations/0/id/typeAnnotation/typeAnnotation/type")
            .and_then(Value::as_str),
        Some("TSTypeQuery"),
        "`typeof x` stays a TSTypeQuery: {json}"
    );
}

/// `import('m')` stays a `TSImportType` (the `Import` keyword arm still wins).
#[test]
fn import_type_unaffected() {
    let json = parse_json("let z: import('m');");
    assert_eq!(
        json.pointer("/body/0/declarations/0/id/typeAnnotation/typeAnnotation/type")
            .and_then(Value::as_str),
        Some("TSImportType"),
        "`import('m')` stays a TSImportType: {json}"
    );
}

/// `x as const;` still parses (the `Const` keyword arm predates and is unaffected).
#[test]
fn as_const_unaffected() {
    let canonical = "x as const;\n";
    assert_eq!(format(canonical), canonical, "idempotent");
}

/// A genuine non-type token after `:` is still a parse error — the new arm only
/// covers keyword tokens, so `;` and `,` still hit the catch-all reject.
#[test]
fn empty_and_bad_type_positions_still_reject() {
    assert!(rejects("let x: ;"), "`let x: ;` is still a parse error");
    assert!(rejects("let x: ,"), "`let x: ,` is still a parse error");
}
