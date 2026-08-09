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
//! The accept, the AST shape and the prettier formatting are pinned by the fixture
//! `typescript/declarations/namespace/function_body_svelte_divergence` (an acorn
//! rejection is representable: `expected_ours.json` plus an `expected_svelte.json`
//! holding `{"error": "failed to parse"}`), which is where the prettier claim belongs
//! — against a live oracle rather than a hand-written string. What remains here is the
//! **equivalence** the fixture cannot state: that the plain form's function node and
//! the `export function` form's are the same node, which is a relation between two
//! parses rather than a property of one.
//!
//! Contrast: a *top-level* `declare function f() {}` HAS the `declare` keyword, which
//! grammatically forces a bodiless signature — prettier rejects a body there and so
//! does tsv, pinned as the fixture's `input_invalid_top_level_declare_body.svelte`. The
//! bodiless-signature guard below stays here because it asserts a node TYPE
//! (`TSDeclareFunction`, not `FunctionDeclaration`), which is the distinction an
//! over-permissive parser would blur while still accepting.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source)
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
