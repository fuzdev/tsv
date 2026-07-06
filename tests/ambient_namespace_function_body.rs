// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A plain `function f() {}` inside a `declare namespace`/`module` body carries no
//! `declare` keyword of its own, so it is an ordinary function *declaration with a
//! body* — its ambient-context violation (tsc TS1183 "An implementation cannot be
//! declared in ambient contexts") is a static-semantic early-error tsv **defers** to
//! the diagnostics layer (see `crates/tsv_ts/CLAUDE.md` §Sources of truth). prettier
//! formats it, so the formatter must parse it; tsv already accepts the sibling
//! `export function f() {}` form, so the plain form must parse identically.
//!
//! This can't be a fixture: Svelte/acorn-typescript *reject* the input (TS1183), so
//! no `expected.json` oracle exists — the shape is pinned here against tsv itself and
//! prettier's formatting.
//!
//! Contrast: a *top-level* `declare function f() {}` HAS the `declare` keyword, which
//! grammatically forces a bodiless signature — prettier rejects a body there, and tsv
//! keeps rejecting it (`bodiless_*` guards below).

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

/// `declare namespace N { function f() {} }` parses, and the inner function is a
/// `FunctionDeclaration` with a `BlockStatement` body (NOT a bodiless
/// `TSDeclareFunction`) — the same node the `export function f() {}` form yields.
#[test]
fn namespace_function_body_parses_as_function_declaration() {
    let json = parse_json("declare namespace N { function f() {} }");
    let f = "/body/0/body/body/0";
    assert_eq!(
        json.pointer(&format!("{f}/type")).and_then(Value::as_str),
        Some("FunctionDeclaration"),
        "plain ambient function with a body is a FunctionDeclaration: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{f}/body/type"))
            .and_then(Value::as_str),
        Some("BlockStatement"),
        "the body is preserved (not dropped as a signature): {json}"
    );
}

/// The `module` keyword spelling behaves identically.
#[test]
fn ambient_module_function_body_parses_as_function_declaration() {
    let json = parse_json("declare module M { function f() {} }");
    assert_eq!(
        json.pointer("/body/0/body/body/0/type")
            .and_then(Value::as_str),
        Some("FunctionDeclaration"),
        "plain ambient module function with a body is a FunctionDeclaration: {json}"
    );
}

/// The plain form's function node matches the `export function` form's (minus the
/// `ExportNamedDeclaration` wrapper) — they must not diverge.
#[test]
fn namespace_function_body_matches_export_form() {
    let plain = parse_json("declare namespace N { function f() {} }");
    let exported = parse_json("declare namespace N { export function f() {} }");
    let plain_fn = plain.pointer("/body/0/body/body/0");
    let exported_fn = exported.pointer("/body/0/body/body/0/declaration");
    assert_eq!(
        plain_fn.and_then(|v| v.get("type")).and_then(Value::as_str),
        Some("FunctionDeclaration")
    );
    assert_eq!(
        exported_fn
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str),
        Some("FunctionDeclaration"),
        "export form is the consistency target"
    );
}

/// prettier formats the plain form to the tab-indented body form; tsv must match and
/// be idempotent (no data loss — the body survives the round-trip).
#[test]
fn namespace_function_body_formats_to_prettier_canonical() {
    let expected = "declare namespace N {\n\tfunction f() {}\n}\n";
    assert_eq!(
        format("declare namespace N { function f() {} }"),
        expected,
        "compact input formats to the canonical body form"
    );
    assert_eq!(format(expected), expected, "idempotent");
}

/// Regression guard: a *bodiless* signature inside a namespace stays a
/// `TSDeclareFunction` (the existing `declarations/namespace/declare` fixture form).
#[test]
fn bodiless_namespace_signature_stays_tsdeclarefunction() {
    let json = parse_json("declare namespace N { function f(): void; }");
    assert_eq!(
        json.pointer("/body/0/body/body/0/type")
            .and_then(Value::as_str),
        Some("TSDeclareFunction"),
        "a `;`-terminated signature is still a bodiless TSDeclareFunction: {json}"
    );
}

/// Regression guard: a *top-level* `declare function f() {}` (the `declare` keyword
/// grammatically forces bodiless) still REJECTS a body — prettier rejects it too.
#[test]
fn top_level_declare_function_body_still_rejected() {
    let arena = bumpalo::Bump::new();
    assert!(
        tsv_ts::parse("declare function f() {}", &arena).is_err(),
        "top-level `declare function` with a body stays a parse error"
    );
}
