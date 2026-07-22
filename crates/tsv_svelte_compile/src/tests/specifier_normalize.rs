//! esrap's import/export specifier printing: the `as` alias is dropped whenever a
//! name side is a string `Literal`, keeping only the identifier binding. Mirrored
//! by [`crate::specifier_normalize`] over the server-emitted module body and the
//! hoisted instance imports. Each accepting case pins the surviving code AND the
//! dropped alias; the parity against the oracle is pinned by the compile fixtures
//! (`tests/fixtures_compile/module/{export,import}_string*`,
//! `imports/string_name`).

use super::support::*;

#[test]
fn compile_module_export_string_alias_drops_alias() {
    // `export { x as 'notdefault' }` → esrap keeps only the local binding `x`.
    let js =
        compile_js("<script module>\n\tconst x = 1;\n\texport { x as 'notdefault' };\n</script>");
    assert!(
        js.contains("export { x };") && !js.contains("notdefault"),
        "the string exported alias must drop to the bare local: {js}"
    );
}

#[test]
fn compile_module_reexport_string_local_drops_alias() {
    // `export { 'a-b' as notdefault } from` → the string local survives, the
    // identifier alias drops (esrap keeps the local side, string or not).
    let js =
        compile_js("<script module>\n\texport { 'a-b' as notdefault } from './y.js';\n</script>");
    assert!(
        js.contains("export { 'a-b' } from './y.js';") && !js.contains("notdefault"),
        "the string local re-export must drop its alias: {js}"
    );
}

#[test]
fn compile_module_reexport_string_both_drops_alias() {
    // `export { 'a-b' as 'c-d' } from` → both sides strings; the local survives.
    let js = compile_js("<script module>\n\texport { 'a-b' as 'c-d' } from './y.js';\n</script>");
    assert!(
        js.contains("export { 'a-b' } from './y.js';") && !js.contains("c-d"),
        "a both-string re-export must keep only the local: {js}"
    );
}

#[test]
fn compile_module_import_string_name_drops_alias() {
    // Module `import { 'a-b' as loc }` → esrap keeps only the local binding `loc`.
    let js = compile_js("<script module>\n\timport { 'a-b' as loc } from './y.js';\n</script>");
    assert!(
        js.contains("import { loc } from './y.js';") && !js.contains("a-b"),
        "the string imported name must drop to the bare binding: {js}"
    );
}

#[test]
fn compile_instance_import_string_name_drops_alias() {
    // The instance-script path reaches the SAME normalization via the hoisted
    // imports (a distinct call site from the module body). `loc` is used in the
    // template so the binding is live.
    let js = compile_js("<script>\n\timport { 'a-b' as loc } from './y.js';\n</script>\n{loc}");
    assert!(
        js.contains("import { loc } from './y.js';") && !js.contains("a-b"),
        "an instance string import must drop its alias too: {js}"
    );
}

#[test]
fn compile_identifier_alias_stays_verbatim() {
    // CONTROL: both sides identifiers → the alias is authorship, kept verbatim.
    // The transform must fire ONLY when a `Literal` is involved.
    let import_js =
        compile_js("<script>\n\timport { thing as loc } from './y.js';\n</script>\n{loc}");
    assert!(
        import_js.contains("import { thing as loc } from './y.js';"),
        "an identifier import alias must stay verbatim: {import_js}"
    );
    let export_js =
        compile_js("<script module>\n\tconst x = 1;\n\texport { x as notdefault };\n</script>");
    assert!(
        export_js.contains("export { x as notdefault };"),
        "an identifier export alias must stay verbatim: {export_js}"
    );
}

#[test]
fn compile_export_star_string_stays_verbatim() {
    // CONTROL: `export * as 'str'` is an `ExportAllDeclaration`, not a specifier —
    // esrap keeps its name verbatim, so the transform deliberately skips it.
    let js = compile_js("<script module>\n\texport * as 'str' from './y.js';\n</script>");
    assert!(
        js.contains("export * as 'str' from './y.js';"),
        "export * as 'str' must keep its string name: {js}"
    );
}

#[test]
fn compile_module_export_string_alias_as_gap_comment_refuses() {
    // A KEPT module comment (a preceding block-bearing `function f()` triggers the
    // F1 carry) sitting in the `as`-gap of a collapsing string-alias export: esrap
    // drops the alias but KEEPS the comment (`export { x /* keep */ };`), while
    // tsv's span collapse makes the specifier printer skip the gap and DROP the
    // comment — a content-loss MISMATCH. Refuse rather than diverge.
    assert_unsupported(
        "<script module>\n\tfunction f() {}\n\tconst x = 1;\n\texport { x /* keep */ as 'y' };\n</script>",
        "comment in a string-specifier as-gap",
    );
}

#[test]
fn compile_module_import_string_name_as_gap_comment_refuses() {
    // The MODULE-import analog: a kept comment in a collapsing string-import's
    // `as`-gap (`import { 'a-b' /* c */ as loc }` → `import { loc }`) would be
    // dropped by the collapse. Module imports stay in the module-body program (they
    // do not hoist to the comment-free instance-import scaffold), so they are guarded
    // too.
    assert_unsupported(
        "<script module>\n\tfunction f() {}\n\timport { 'a-b' /* c */ as loc } from './y.js';\n</script>",
        "comment in a string-specifier as-gap",
    );
}

#[test]
fn compile_module_export_string_alias_no_block_comment_compiles() {
    // CONTROL: WITHOUT a preceding block the comment is NOT carried (the F1 keep set
    // is empty), so BOTH the oracle and tsv drop it (parity) — the guard must NOT
    // refuse this common shape. It still compiles.
    let js = compile_js("<script module>const x = 1;export { x /* keep */ as 'y' };</script>");
    assert!(
        js.contains("export { x };") && !js.contains("keep"),
        "the no-block string alias must compile, dropping the comment on both sides: {js}"
    );
}

#[test]
fn compile_identifier_alias_as_gap_comment_compiles() {
    // CONTROL: an IDENTIFIER alias (no `Literal`) does not collapse, so the printer
    // keeps `x as y` and emits the gap comment — a kept comment reaches parity, no
    // refusal. The guard fires only when a specifier actually collapses.
    let js = compile_js(
        "<script module>\n\tfunction f() {}\n\tconst x = 1;\n\texport { x /* keep */ as y };\n</script>",
    );
    assert!(
        js.contains("as y") && js.contains("keep"),
        "the identifier alias with a gap comment must compile, keeping both: {js}"
    );
}

#[test]
fn compile_identifier_self_alias_untouched_by_transform() {
    // The `x as x` identifier self-export is a SEPARATE, main-owed gap: tsv's
    // printer does not structurally collapse it (the oracle does), so it stays a
    // corpus-absent mismatch. This test only pins that the string-specifier
    // transform LEAVES it alone — the output is tsv's own `x as x` (a self-
    // consistent canonicalize fixed point), never accidentally collapsed here.
    let js = compile_js("<script module>\n\tconst x = 1;\n\texport { x as x };\n</script>");
    assert!(
        js.contains("export { x as x };"),
        "the string transform must not touch an identifier self-alias: {js}"
    );
}
