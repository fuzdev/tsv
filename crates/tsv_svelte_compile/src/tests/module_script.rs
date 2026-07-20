//! The `<script module>` refusals and comment ordering.

use super::support::*;

#[test]
fn compile_module_refuses_default_export() {
    // `export default` in a module is the oracle's `module_illegal_default_export`
    // error — refuse rather than emit.
    assert_unsupported(
        "<script module>\n\texport default 5;\n</script>\n<p>hi</p>",
        "default export in <script module>",
    );
}

#[test]
fn compile_module_refuses_state_rune() {
    // v1 defers the oracle's module `$state`→v rewrite (the corpus is module-rune-
    // free), so a module-scope rune refuses via the guard — a safe over-refusal.
    assert_unsupported(
        "<script module>\n\tlet count = $state(0);\n</script>\n<p>hi</p>",
        "rune $state",
    );
}

#[test]
fn compile_module_refuses_store_read() {
    // A module-scope `$name` store read is the oracle's `store_invalid_subscription`
    // error — the guard refuses it (no store exemption in a module).
    assert_unsupported(
        "<script module>\n\timport { writable } from 'svelte/store';\n\tconst c = writable(0);\n\tconst v = $c;\n</script>\n<p>hi</p>",
        "$-prefixed identifier $c",
    );
}

#[test]
fn compile_module_refuses_top_level_await() {
    // Top-level `await` forces the oracle's async-component shapes (not implemented),
    // so a module top-level await refuses — a safe over-refusal (the oracle compiles it).
    assert_unsupported(
        "<script module>\n\tconst x = await fetch('/');\n</script>\n<p>hi</p>",
        "top-level await",
    );
}

#[test]
fn compile_module_body_follows_hoisted_snippet() {
    // Emission order (probe-verified): the module block prints AFTER the hoisted
    // snippets, NOT merged into the instance import group — imports, hoisted
    // snippet, module body, then the component function.
    let js = compile_js(
        "<script module>\n\tconst SHARED = 5;\n</script>\n{#snippet foo()}<p>{SHARED}</p>{/snippet}\n{@render foo()}",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             function foo($$renderer) {\n\
             \t$$renderer.push(`<p>5</p>`);\n\
             }\n\
             const SHARED = 5;\n\
             export default function Input($$renderer) {\n\
             \tfoo($$renderer);\n\
             }\n"
    );
}

#[test]
fn compile_module_sets_document_ts_flag() {
    // A `lang="ts"` module sets the document-wide TypeScript flag, so the instance
    // script's TypeScript erases even though it carries no `lang` of its own.
    let js = compile_js(
        "<script module lang=\"ts\">\n\tconst K: number = 5;\n</script>\n<script>\n\tlet a: number = 1;\n</script>\n<p>{a}{K}</p>",
    );
    assert!(
        !js.contains(": number"),
        "instance TypeScript must erase under the module's lang=\"ts\": {js}"
    );
}

#[test]
fn compile_module_refuses_name_collision_with_instance() {
    // A name declared in BOTH scripts: the oracle resolves `{K}` to the instance
    // (inner-scope) binding (`$.escape(K)`), but the name-based table would fold
    // the module `const K = 5` — a real MISMATCH, so refuse.
    assert_unsupported(
        "<script module>\n\tconst K = 5;\n</script>\n<script>\n\tlet { K } = $props();\n</script>\n<p>{K}</p>",
        "declared in both the module and instance scripts",
    );
}

#[test]
fn compile_module_before_instance_comment_carries() {
    // A whitespace-only text run between the module `</script>` and the instance
    // `<script>` must NOT trip the template-before-script comment guard — the
    // instance comment carries through (parity with the oracle).
    let js = compile_js(
        "<script module>\n\tconst K = 5;\n</script>\n\n<script>\n\t// instance comment\n\tlet a = 1;\n</script>\n<p>{a}{K}</p>",
    );
    assert!(
        js.contains("// instance comment"),
        "the instance comment must carry through past the module script: {js}"
    );
}

#[test]
fn compile_refuses_module_comment_after_instance_script() {
    // A module script placed AFTER the instance script puts its comments at
    // offsets the oracle's printer re-seeks BACKWARD over (the component body
    // block carries the instance script's `loc`), so esrap re-attaches them into
    // whatever loc-bearing node it reaches next — a template expression it has
    // nothing to do with. tsv drops the comment, which is a comment PRESENCE
    // difference the parity bar grades as a MISMATCH. Refuse.
    for source in [
        // The minimal shape: instance script, module script, template expression.
        "<script>function w(x){return x;}</script><script module>\n// MYC\nconst K = 5;\n</script>{w(1)}",
        // A block comment lands the same way.
        "<script>function w(x){return x;}</script><script module>\n/* MYC */\nconst K = 5;\n</script>{w(1)}",
        // The comment past the module body's last statement lands the same way.
        "<script>function w(x){return x;}</script><script module>\nconst K = 5;\n// MYC\n</script>{w(1)}",
        // An import-only instance script still supplies the `loc` that seeks back.
        "<script>import {a} from './a.js';</script><script module>\n// MYC\nconst K = 5;\n</script>{a}",
    ] {
        assert_unsupported(source, "module script placed after the instance script");
    }
}

#[test]
fn compile_module_comment_before_instance_script_still_drops() {
    // The mirror of the refusal above: with the module script FIRST, the body
    // block's seek moves FORWARD past the module comment, so the oracle drops it
    // too — tsv's drop is parity and must keep compiling.
    let js = compile_js(
        "<script module>\n// MYC\nconst K = 5;\n</script><script>function w(x){return x;}</script>{w(1)}",
    );
    assert!(
        !js.contains("MYC"),
        "a module comment before the instance script must drop: {js}"
    );
}

#[test]
fn compile_rejects_exporting_a_non_hoistable_snippet() {
    // `snippet_invalid_export` (`2-analyze/index.js:831`): the snippet references
    // the INSTANCE script, so the oracle cannot hoist it into module scope and the
    // export names nothing there.
    assert_unsupported(
        "<script module>export { foo };</script><script>let x = 42;</script>{#snippet foo()}{x}{/snippet}",
        "exported {#snippet} foo is not module-hoistable",
    );
    // A snippet below the root fragment never hoists either — and the oracle still
    // reports the SPECIFIC error, because `analysis.snippets` is unfiltered by
    // top-level-ness.
    assert_unsupported(
        "<script module>export { foo };</script><div>{#snippet foo()}s{/snippet}</div>",
        "exported {#snippet} foo is not module-hoistable",
    );
}

#[test]
fn compile_rejects_exporting_an_undeclared_name() {
    // `export_undefined` (`2-analyze/index.js:833`).
    assert_unsupported(
        "<script module>export { blah };</script>",
        "module script exports blah, which it does not declare",
    );
    // ⚠️ An INSTANCE declaration does not count: the instance scope is a CHILD of
    // the module scope, and `scope.get` never walks down. Live-probed.
    assert_unsupported(
        "<script module>export { x };</script><script>let x = 1;</script>",
        "module script exports x, which it does not declare",
    );
}

#[test]
fn compile_accepts_the_module_export_rule_s_exemptions() {
    // ⚠️ The hoist interaction: a hoistable top-level snippet's binding is written
    // INTO the module scope (`SnippetBlock.js:40-44`), so the export resolves and
    // there is no error — the reason module scope must be consulted BEFORE the
    // snippet-name set. Live-probed, and the case `checklist_svelte_compiler.md`
    // previously described the wrong way round.
    let _ = compile_js("<script module>export { foo };</script>{#snippet foo()}static{/snippet}");
    // A snippet referencing only the MODULE script still hoists.
    let _ = compile_js(
        "<script module>export { foo };\nlet m = 1;</script>{#snippet foo()}{m}{/snippet}",
    );
    // Every ordinary module declaration form resolves.
    let _ = compile_js("<script module>const a = 1;\nexport { a as b };</script>");
    let _ = compile_js("<script module>function f() {}\nexport { f };</script>");
    let _ = compile_js("<script module>import { q } from 'y';\nexport { q };</script>");
    // `export … from 'y'` is exempt (the oracle's `node.source == null` gate).
    let _ = compile_js("<script module>export { x } from 'y';</script>");
    // A type-only export never reaches the rule: erasure drops it, exactly as the
    // oracle's own phase-1 `remove_typescript_nodes` does.
    let _ = compile_js("<script module lang=\"ts\">type Foo = 1;\nexport type { Foo };</script>");
    let _ =
        compile_js("<script module lang=\"ts\">interface Foo {}\nexport { type Foo };</script>");
}
