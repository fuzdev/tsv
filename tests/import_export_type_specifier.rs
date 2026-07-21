// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Type-only import/export specifier disambiguation: a leading contextual `type`
//! may be the type-only modifier (`{ type A }`) or the imported/local name itself
//! (`{ type as age }` — a value import/export of a binding named `type`,
//! renamed). tsv used to over-reject `{ type as <name> }` and `{ type as as }`
//! (reading `type` as the modifier when an `as` followed). These pin acorn's
//! `parseTypeOnlyImportExportSpecifier` state machine directly on the wire AST —
//! including `{ type as as }` (bare), which can't share a fixture with
//! `{ type as }` (both bind local `as`, a duplicate the canonical parser rejects).
//!
//! Verified against tsc 5.9.2 (the authoritative oracle) via its own conformance
//! cases `externalModules/typeOnly/{importSpecifiers1,exportSpecifiers}.ts`.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source)
}

/// Assert the first import specifier's `imported`/`local` names and `importKind`.
fn assert_import_spec(source: &str, imported: &str, local: &str, kind: &str) {
    let json = parse_json(source);
    let spec = json
        .pointer("/body/0/specifiers/0")
        .expect("import specifier 0");
    assert_eq!(
        spec.pointer("/imported/name").and_then(Value::as_str),
        Some(imported),
        "imported name for `{source}`: {json}"
    );
    assert_eq!(
        spec.pointer("/local/name").and_then(Value::as_str),
        Some(local),
        "local name for `{source}`: {json}"
    );
    assert_eq!(
        spec.get("importKind").and_then(Value::as_str),
        Some(kind),
        "importKind for `{source}`: {json}"
    );
}

/// Assert the first export specifier's `local`/`exported` names and `exportKind`.
fn assert_export_spec(source: &str, local: &str, exported: &str, kind: &str) {
    let json = parse_json(source);
    let spec = json
        .pointer("/body/0/specifiers/0")
        .expect("export specifier 0");
    assert_eq!(
        spec.pointer("/local/name").and_then(Value::as_str),
        Some(local),
        "local name for `{source}`: {json}"
    );
    assert_eq!(
        spec.pointer("/exported/name").and_then(Value::as_str),
        Some(exported),
        "exported name for `{source}`: {json}"
    );
    assert_eq!(
        spec.get("exportKind").and_then(Value::as_str),
        Some(kind),
        "exportKind for `{source}`: {json}"
    );
}

// ── Imports ────────────────────────────────────────────────────────────────

/// The tracked gap: `type as <name>` (one `as`) is a VALUE import of `type`,
/// renamed — NOT the type-only modifier.
#[test]
fn import_type_as_name_is_value_rename() {
    assert_import_spec("import { type as age } from 'y'", "type", "age", "value");
}

/// `type as as` (bare, no trailing name) is a VALUE import of `type`, renamed to
/// the local `as` — the second flip tsv used to reject.
#[test]
fn import_type_as_as_bare_is_value_rename_to_as() {
    assert_import_spec("import { type as as } from 'y'", "type", "as", "value");
}

/// `type as` (bare) is a TYPE-only import of a binding named `as`.
#[test]
fn import_type_as_bare_is_type_only_of_as() {
    assert_import_spec("import { type as } from 'y'", "as", "as", "type");
}

/// `type as as <name>`: `type` is the modifier, `as` the imported name, renamed.
#[test]
fn import_type_as_as_name_is_type_of_as_renamed() {
    assert_import_spec("import { type as as year } from 'y'", "as", "year", "type");
}

/// `type <name>` is the type-only modifier.
#[test]
fn import_type_name_is_modifier() {
    assert_import_spec("import { type Foo } from 'y'", "Foo", "Foo", "type");
}

/// Bare `type` is a value import of a binding named `type`.
#[test]
fn import_type_bare_is_value_name() {
    assert_import_spec("import { type } from 'y'", "type", "type", "value");
}

// ── Exports (mirror the same state machine, `isImport = false`) ──────────────

/// `export { type as age }` is a VALUE export of the local `type`, renamed.
#[test]
fn export_type_as_name_is_value_rename() {
    assert_export_spec("export { type as age }", "type", "age", "value");
}

/// `export { type as as }` (bare) is a VALUE export of `type`, renamed to `as`.
#[test]
fn export_type_as_as_bare_is_value_rename_to_as() {
    assert_export_spec("export { type as as }", "type", "as", "value");
}

/// `export { type as }` (bare) is a TYPE-only export of the local `as`.
#[test]
fn export_type_as_bare_is_type_only_of_as() {
    assert_export_spec("export { type as }", "as", "as", "type");
}

/// `export { type Foo }` is the type-only modifier; bare `export { type }` a value.
#[test]
fn export_type_modifier_and_bare_value() {
    assert_export_spec("export { type Foo }", "Foo", "Foo", "type");
    assert_export_spec("export { type }", "type", "type", "value");
}
