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
fn compile_refuses_invalid_script_context() {
    // The oracle's parse-time `script_invalid_context` (read/script.js:66-78): a
    // `context` attribute is valid ONLY as the text "module". Any other text.
    assert_unsupported(
        "<script context=\"foo\">\n\tlet x = 1;\n</script>\n<p>{x}</p>",
        "context attribute other than context=\"module\"",
    );
    // `context="default"` — a plausible mistake, still rejected.
    assert_unsupported(
        "<script context=\"default\">\n\tlet x = 1;\n</script>\n<p>{x}</p>",
        "context attribute other than context=\"module\"",
    );
    // A boolean `context` (no value).
    assert_unsupported(
        "<script context>\n\tlet x = 1;\n</script>\n<p>{x}</p>",
        "context attribute other than context=\"module\"",
    );
    // An expression value `context={…}` — not a text attribute.
    assert_unsupported(
        "<script context={foo}>\n\tlet x = 1;\n</script>\n<p>{x}</p>",
        "context attribute other than context=\"module\"",
    );
    // Checked on the MODULE script too: `<script module context="foo">` is a module
    // script to tsv (the boolean `module`), but the oracle still rejects the bad
    // `context`.
    assert_unsupported(
        "<script module context=\"foo\">\n\tconst a = 1;\n</script>\n<p>x</p>",
        "context attribute other than context=\"module\"",
    );
    // Discriminating controls, all COMPILE: the legacy `context="module"`, the
    // modern `module`, and a plain instance script.
    let _ = compile_js("<script context=\"module\">const a = 1;</script>\n<p>x</p>");
    let _ = compile_js("<script module>const a = 1;</script>\n<p>x</p>");
    let _ = compile_js("<script>let x = 1;</script>\n<p>{x}</p>");
}

#[test]
fn compile_refuses_valued_module_attribute() {
    // The oracle's parse-time `script_invalid_attribute_value` (read/script.js:57-64):
    // the `module` attribute must be a plain BOOLEAN; any value refuses.
    assert_unsupported(
        "<script module=\"foo\">\n\tconst a = 1;\n</script>\n<p>x</p>",
        "<script module> attribute with a value",
    );
    // Even `module="module"` refuses — the VALUE is what's illegal, not the text.
    assert_unsupported(
        "<script module=\"module\">\n\tconst a = 1;\n</script>\n<p>x</p>",
        "<script module> attribute with a value",
    );
    // An empty string value.
    assert_unsupported(
        "<script module=\"\">\n\tconst a = 1;\n</script>\n<p>x</p>",
        "<script module> attribute with a value",
    );
    // A `{…}` value.
    assert_unsupported(
        "<script module={x}>\n\tconst a = 1;\n</script>\n<p>x</p>",
        "<script module> attribute with a value",
    );
    // Mixed attributes — the shared source-order pass matches the oracle's
    // first-error-wins: `module` first reports the module-value rule…
    assert_unsupported(
        "<script module=\"bar\" context=\"foo\">\n\tconst a = 1;\n</script>\n<p>x</p>",
        "<script module> attribute with a value",
    );
    // …`context` first reports the context rule.
    assert_unsupported(
        "<script context=\"foo\" module=\"bar\">\n\tconst a = 1;\n</script>\n<p>x</p>",
        "context attribute other than context=\"module\"",
    );
    // Control: the bare boolean `module` is the valid module-script spelling.
    let _ = compile_js("<script module>const a = 1;</script>\n<p>x</p>");
}

#[test]
fn compile_refuses_reserved_script_attribute() {
    // The oracle's parse-time `script_reserved_attribute` (read/script.js:49-51):
    // the FIRST check in the attribute loop — a `<script>` attribute named
    // server/client/worker/test/default is rejected. Each name is pinned to prove
    // the carried `{name}` is faithful.
    for name in ["server", "client", "worker", "test", "default"] {
        assert_unsupported(
            &format!("<script {name}>\n\tlet x = 1;\n</script>\n<p>{{x}}</p>"),
            &format!("reserved <script> attribute {name}"),
        );
    }
    // The check fires regardless of the attribute's VALUE — `server="x"` is still
    // reserved (unlike `module`, where the value is the illegal part).
    assert_unsupported(
        "<script server=\"x\">\n\tlet x = 1;\n</script>\n<p>{x}</p>",
        "reserved <script> attribute server",
    );
    // Reserved is checked on a MODULE-classified script too: the boolean `module`
    // routes it to the module slot, but its `server` attribute still rejects.
    assert_unsupported(
        "<script module server>\n\tconst a = 1;\n</script>\n<p>x</p>",
        "reserved <script> attribute server",
    );
    // First-error-wins ordering within a script: reserved BEFORE a valid module…
    assert_unsupported(
        "<script server module>\n\tconst a = 1;\n</script>\n<p>x</p>",
        "reserved <script> attribute server",
    );
    // …but a source-earlier `module="x"` value error still reports first.
    assert_unsupported(
        "<script module=\"x\" server>\n\tconst a = 1;\n</script>\n<p>x</p>",
        "<script module> attribute with a value",
    );
    // …and a source-earlier reserved name beats a later module value error.
    assert_unsupported(
        "<script server module=\"x\">\n\tconst a = 1;\n</script>\n<p>x</p>",
        "reserved <script> attribute server",
    );
    // Discriminating controls, all COMPILE: an UNKNOWN attribute is only a warning
    // (`script_unknown_attribute`), never the reserved error — the closed reserved
    // set must not swallow it. `servers` is not `server`; `foo`/`foo="x"` are plain
    // unknowns; the allowed `lang="ts"` is neither reserved nor unknown.
    let _ = compile_js("<script servers>let x = 1;</script>\n<p>{x}</p>");
    let _ = compile_js("<script foo>let x = 1;</script>\n<p>{x}</p>");
    let _ = compile_js("<script foo=\"x\">let x = 1;</script>\n<p>{x}</p>");
    let _ = compile_js("<script lang=\"ts\">let x: number = 1;</script>\n<p>{x}</p>");
}

#[test]
fn compile_module_refuses_export_as_default() {
    // The oracle's single `module_illegal_default_export` fires from its
    // `ExportNamedDeclaration` visitor too: an `export { x as default }` specifier
    // (`ExportNamedDeclaration.js:15-23`). Identifier form.
    assert_unsupported(
        "<script module>\n\tlet answer = 42;\n\texport { answer as default };\n</script>\n<p>hi</p>",
        "default export in <script module>",
    );
    // String-literal alias `as \"default\"` — the oracle's `.value === 'default'`
    // arm. tsv's parser accepts the arbitrary-module-name form, so it must refuse.
    assert_unsupported(
        "<script module>\n\tlet answer = 42;\n\texport { answer as \"default\" };\n</script>\n<p>hi</p>",
        "default export in <script module>",
    );
    // The named default check is NOT gated on `node.source`, so a re-export
    // `export { x as default } from 'y'` refuses too — unlike the snippet-export
    // rule, which exempts a re-export.
    assert_unsupported(
        "<script module>\n\texport { foo as default } from './y.js';\n</script>\n<p>hi</p>",
        "default export in <script module>",
    );
    // An ESCAPED identifier alias that decodes to `default` — the oracle compares
    // the DECODED `.name`, so it rejects this too. `Identifier::name` reads the
    // decoded `escaped_name`, so tsv catches it (a plain-source-slice read would miss it).
    assert_unsupported(
        "<script module>\n\tlet answer = 42;\n\texport { answer as \\u0064efault };\n</script>\n<p>hi</p>",
        "default export in <script module>",
    );
    // Discriminating controls: a non-`default` alias compiles on both paths (the
    // rule keys on the exported name being `default`, not on aliasing itself) —
    // including an escaped alias that decodes to something OTHER than `default`.
    let _ = compile_js("<script module>let answer = 42;\nexport { answer as other };</script>");
    let _ = compile_js("<script module>let answer = 42;\nexport { answer as \\u0066oo };</script>");
    let _ = compile_js("<script module>export { foo as bar } from './y.js';</script>");
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

// ── The "open half": a module comment the oracle KEEPS, recovered by esrap's
// comment-index re-seek over a preceding block-bearing statement. The keep
// condition (both must hold): (1) a `BlockStatement`/`ClassBody`/static block
// STARTS before the comment; (2) a flush target exists — a non-empty module
// statement extending past the comment, OR an instance script. Each case is
// derived from `canonical_compile`, and paired with a discriminating drop so the
// assertion cannot pass vacuously.

#[test]
fn compile_module_comment_after_block_carries() {
    // A `function` declaration (its body block) precedes the comment, and a later
    // module statement flushes it — the oracle keeps it, so tsv must too.
    let js = compile_js(
        "<script module>function f(){}\n// MODMARK\nexport const g = 2;\n</script><p>hi</p>",
    );
    assert!(
        js.contains("// MODMARK"),
        "a module comment after a block must carry: {js}"
    );
}

#[test]
fn compile_module_comment_before_block_drops() {
    // The discriminating control for the case above: the SAME block, but the
    // comment sits BEFORE it — so esrap's re-seek moves past the comment and the
    // oracle drops it. tsv must drop too (dropping one the oracle keeps, or keeping
    // one it drops, are both mismatches).
    let js = compile_js(
        "<script module>// MODMARK\nfunction f(){}\nexport const g = 2;\n</script><p>hi</p>",
    );
    assert!(
        !js.contains("MODMARK"),
        "a module comment before every block must drop: {js}"
    );
}

#[test]
fn compile_module_comment_no_block_drops() {
    // No block at all (a plain `const` init): condition 1 fails, so the comment
    // drops even with a flush target present.
    let js = compile_js(
        "<script module>const a = 1;\n// MODMARK\nexport const b = 2;\n</script><p>hi</p>",
    );
    assert!(
        !js.contains("MODMARK"),
        "a module comment with no preceding block must drop: {js}"
    );
}

#[test]
fn compile_module_comment_arrow_expression_drops() {
    // An arrow with an EXPRESSION body has no `BlockStatement`, so it is not a
    // block — the comment drops. (An arrow with a `{}` block body WOULD keep it.)
    let js = compile_js(
        "<script module>const f = () => 1;\n// MODMARK\nexport const g = f();\n</script><p>hi</p>",
    );
    assert!(
        !js.contains("MODMARK"),
        "an arrow expression body is not a block; the comment must drop: {js}"
    );
}

#[test]
fn compile_module_comment_switch_drops() {
    // A `switch` has no `BlockStatement` node (its braces are syntactic), so it does
    // not trigger the re-seek — the comment drops.
    let js = compile_js(
        "<script module>switch (1) { case 1: break; }\n// MODMARK\nexport const g = 2;\n</script><p>hi</p>",
    );
    assert!(
        !js.contains("MODMARK"),
        "a switch is not a block; the comment must drop: {js}"
    );
}

#[test]
fn compile_module_comment_class_body_carries() {
    // A `ClassBody` — even a field-only class with no method body — is a block the
    // oracle re-seeks on, so a following comment carries.
    let js = compile_js(
        "<script module>class C { x = 1; }\n// MODMARK\nexport const g = 2;\n</script><p>hi</p>",
    );
    assert!(
        js.contains("// MODMARK"),
        "a comment after a class body must carry: {js}"
    );
}

#[test]
fn compile_module_comment_inside_block_carries() {
    // A comment INSIDE the only block, with no later statement and no instance:
    // condition 1 holds (the `{` precedes it) and the block's closing `}` is the
    // flush target — the oracle keeps it.
    let js = compile_js("<script module>function f(){\n// MODMARK\n}\n</script><p>hi</p>");
    assert!(
        js.contains("// MODMARK"),
        "a comment inside a block (flushed by its close) must carry: {js}"
    );
}

#[test]
fn compile_module_comment_after_last_no_flush_drops() {
    // A block precedes the comment, but the comment is past the last module
    // statement with NO instance script — no flush target, so it drops.
    let js = compile_js("<script module>function f(){}\n// MODMARK\n</script><p>hi</p>");
    assert!(
        !js.contains("MODMARK"),
        "a module comment past the last statement with no flush target must drop: {js}"
    );
}

#[test]
fn compile_module_comment_after_last_with_instance_carries() {
    // The same after-last comment, but WITH an instance script present — the
    // instance supplies the flush, so the oracle keeps it. The oracle re-attaches
    // it into the component signature while tsv keeps it in the module body: a
    // POSITION difference the parity bar tolerates, but the comment is present.
    let js = compile_js(
        "<script module>function f(){}\n// MODMARK\n</script><script>let x = 1;</script><p>{x}</p>",
    );
    assert!(
        js.contains("// MODMARK"),
        "an after-last module comment with an instance script must carry: {js}"
    );
}

#[test]
fn compile_module_comment_in_param_list_before_block_drops() {
    // The block's `{` sits AFTER the comment (the comment is in the parameter
    // list), so the re-seek anchors on the BLOCK's start, not the statement's — the
    // oracle drops it. The discriminating control for `..._after_block_carries`.
    let js = compile_js(
        "<script module>function f(\n// MODMARK\n){}\nexport const g = 2;\n</script><p>hi</p>",
    );
    assert!(
        !js.contains("MODMARK"),
        "a comment before the block (in the param list) must drop: {js}"
    );
}

#[test]
fn compile_module_comment_multi_split_before_and_after_block() {
    // Two comments, one before the block and one after: only the second carries.
    // Confirms the per-comment independence of the keep rule.
    let js = compile_js(
        "<script module>// DROPMARK\nfunction f(){}\n// KEEPMARK\nexport const g = 2;\n</script><p>hi</p>",
    );
    assert!(
        js.contains("// KEEPMARK") && !js.contains("DROPMARK"),
        "the pre-block comment drops and the post-block comment carries: {js}"
    );
}

#[test]
fn compile_module_multiline_block_comment_refuses() {
    // A KEPT module comment that would reprint divergently refuses (safe): esrap
    // re-indents a multi-line block comment's interior lines.
    assert_unsupported(
        "<script module>function f(){}\n/* line one\n\t\tline two */\nexport const g = 2;\n</script><p>hi</p>",
        "multi-line block comment",
    );
}

#[test]
fn compile_module_format_ignore_comment_refuses() {
    // A KEPT module `prettier-ignore` refuses (would switch the printer to
    // raw-source emission of the following statement).
    assert_unsupported(
        "<script module>function f(){}\n// prettier-ignore\nexport const g = 2;\n</script><p>hi</p>",
        "format-ignore directive",
    );
}

#[test]
fn compile_module_comment_in_erased_region_refuses() {
    // A KEPT module comment intersecting an erased TypeScript region refuses — the
    // oracle's surviving placement there is an emergent stale-span artifact.
    assert_unsupported(
        "<script module lang=\"ts\">function f(){}\nexport const g: /* MODMARK */ number = 2;\n</script><p>hi</p>",
        "erased TypeScript region",
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
