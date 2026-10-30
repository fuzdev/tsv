// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! Node-type pins for a `<<` after a `typeof` query: tsc's shift operator, where
//! acorn-typescript splits the token to open a type-argument list.
//!
//! tsc reads a type QUERY's arguments with `tryParseTypeArguments`, which takes a lone `<`
//! only, while a type REFERENCE's list re-scans a `<<` into `<` `<`
//! (`parseTypeArgumentsOfTypeReference`). So `a as typeof b << c` is `(a as typeof b) << c`
//! to tsc, and `a as T << c` does not parse. acorn-typescript splits the `<<` in both
//! positions and rejects both texts. No text reads both ways — acorn's list can only open a
//! generic function type (`typeof f<<T>() => void>`, which only acorn accepts: the
//! `shift_left_typeof_query_prettier_divergence` fixture), whose shift reading puts an arrow in
//! an operand position — so tsv decides the `<<` where it stands, from the bytes after it.
//!
//! The fixture `typescript/expressions/binary/shift_after_typeof_query` pins the readings as a
//! formatting claim — prettier parenthesizes the assertion, so the bare spelling is nobody's
//! fixed point and rides as its `unformatted_bare_operand` variant — with the type-reference
//! contrast as its `input_invalid_*`. This file asserts the trees directly. Cataloged in
//! `docs/conformance_svelte.md` §TypeScript Corrections.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::__private::parse(source, &arena).expect("parse failed");
    tsv_debug::json::wire_value(&tsv_ts::__private::convert_ast_json_bytes(&program, source))
}

fn accepts(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::__private::parse(source, &arena).is_ok()
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::__private::parse(source, &arena).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

/// The `type` of the node at `pointer`, or `<none>` where nothing is.
fn node_type(json: &Value, pointer: &str) -> String {
    json.pointer(pointer)
        .and_then(|n| n.get("type"))
        .and_then(Value::as_str)
        .map_or_else(|| "<none>".to_owned(), str::to_owned)
}

const INIT: &str = "/body/0/declarations/0/init";

/// The shift over the assertion over the argument-less query, and its printed form.
#[test]
fn shift_after_query_is_tsc_shift() {
    let cases = [
        (
            "const a = b as typeof c << d;",
            "TSAsExpression",
            "const a = (b as typeof c) << d;\n",
        ),
        (
            "const a = b satisfies typeof c << d;",
            "TSSatisfiesExpression",
            "const a = (b satisfies typeof c) << d;\n",
        ),
        (
            "const a = b as typeof c.d << e;",
            "TSAsExpression",
            "const a = (b as typeof c.d) << e;\n",
        ),
        // the `<<` on the next line: the shift too
        (
            "const a = b as typeof c\n<< d;",
            "TSAsExpression",
            "const a = (b as typeof c) << d;\n",
        ),
    ];
    let mut failures = Vec::new();
    for (source, assertion, formatted) in cases {
        if !accepts(source) {
            failures.push(format!("{source:?}\n    rejected"));
            continue;
        }
        let json = parse_json(source);
        let got = (
            node_type(&json, INIT),
            json.pointer(&format!("{INIT}/operator"))
                .and_then(Value::as_str)
                .unwrap_or("<none>")
                .to_owned(),
            node_type(&json, &format!("{INIT}/left")),
            node_type(&json, &format!("{INIT}/left/typeAnnotation")),
            json.pointer(&format!("{INIT}/left/typeAnnotation/typeArguments"))
                .is_some_and(|t| !t.is_null()),
        );
        let want = (
            "BinaryExpression".to_owned(),
            "<<".to_owned(),
            assertion.to_owned(),
            "TSTypeQuery".to_owned(),
            false,
        );
        if got != want {
            failures.push(format!("{source:?}\n    got  {got:?}\n    want {want:?}"));
        }
        let out = format(source);
        if out != formatted {
            failures.push(format!(
                "{source:?}\n    formatted {out:?}\n    want      {formatted:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Where the tail opens a generic function type, the `<<` opens acorn-typescript's list, that
/// type its first argument.
#[test]
fn query_list_opened_by_shift_token_is_acorn_list() {
    for (source, pointer) in [
        (
            "type A = typeof f<<T>() => void>;",
            "/body/0/typeAnnotation",
        ),
        (
            "const a = b as typeof f<<T>() => void>;",
            "/body/0/declarations/0/init/typeAnnotation",
        ),
        (
            "let x: typeof f<<T>() => void> = y;",
            "/body/0/declarations/0/id/typeAnnotation/typeAnnotation",
        ),
        // a shift in the same text leaves the list where it stands
        (
            "const a = b as typeof c << d;\ntype A = typeof f<<T>() => void>;",
            "/body/1/typeAnnotation",
        ),
    ] {
        let json = parse_json(source);
        assert_eq!(node_type(&json, pointer), "TSTypeQuery", "{source}");
        assert_eq!(
            node_type(&json, &format!("{pointer}/typeArguments/params/0")),
            "TSFunctionType",
            "{source}"
        );
    }
}

/// A type REFERENCE re-scans the `<<` into a list, which `<< d` cannot complete — tsc,
/// acorn-typescript and tsv all reject.
#[test]
fn shift_after_type_reference_rejects() {
    assert!(!accepts("const a = b as T << d;"));
    assert!(!accepts("const a = b as import('m') << d;"));
}

/// The `<<` is decided where it stands, so a text the parse driver re-reads as acorn does —
/// over a type-argument follower tsc refuses, in another statement — keeps the shift: each
/// statement takes the reading it takes alone. The first is acorn's call (never the comparison
/// over its kept list, which no parser reads), the second tsc's shift.
#[test]
fn shift_after_query_beside_acorn_reread_statement() {
    let source = "f<() => A><U><V>(x);\na as typeof c << d;";
    let json = parse_json(source);
    assert_eq!(node_type(&json, "/body/0/expression"), "CallExpression");
    assert_eq!(node_type(&json, "/body/1/expression"), "BinaryExpression");
    assert_eq!(
        json.pointer("/body/1/expression/operator")
            .and_then(Value::as_str),
        Some("<<")
    );
    assert_eq!(
        format(source),
        "((f<() => A>)<U>)<V>(x);\n(a as typeof c) << d;\n"
    );
}
