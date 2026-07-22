//! `{#snippet}` hoist analysis and `{@render}` emission.

use super::support::*;

#[test]
fn compile_hoistable_snippet_and_render() {
    // A top-level snippet whose only reference is its own parameter hoists to
    // module scope; `{@render foo(1)}` becomes `foo($$renderer, 1)`, standalone
    // (sole child, non-dynamic) so no trailing anchor.
    let js = compile_js("{#snippet foo(x)}<p>{x}</p>{/snippet}\n{@render foo(1)}");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             function foo($$renderer, x) {\n\
             \t$$renderer.push(`<p>${$.escape(x)}</p>`);\n\
             }\n\
             export default function Input($$renderer) {\n\
             \tfoo($$renderer, 1);\n\
             }\n"
    );
}

#[test]
fn compile_non_hoistable_snippet_stays_in_body() {
    // A snippet referencing a prop can't hoist — the `function` declaration
    // stays in the component body, after the props destructure.
    let js = compile_js(
        "<script>let { name } = $props();</script>\n{#snippet foo()}<p>{name}</p>{/snippet}\n{@render foo()}",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { name } = $$props;\n\
             \tfunction foo($$renderer) {\n\
             \t\t$$renderer.push(`<p>${$.escape(name)}</p>`);\n\
             \t}\n\
             \tfoo($$renderer);\n\
             }\n"
    );
}

#[test]
fn compile_snippet_component_spread_reference_blocks_hoist() {
    // The regression shape: a snippet whose ONLY instance-binding reference
    // sits in a component `{...spread}` must NOT module-hoist (a hoisted
    // `function s` referencing `n` declared inside Input is a runtime
    // ReferenceError — invisible to the reparse self-validation). The
    // shared attr_refs traversal makes the hoist collector see the spread.
    let js = compile_js(
        "<script>import Foo from './Foo.svelte';\n\tlet n = $state({ a: 1 });</script>\n{#snippet s()}<Foo {...n} />{/snippet}\n{@render s()}",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             import Foo from './Foo.svelte';\n\
             export default function Input($$renderer) {\n\
             \tlet n = { a: 1 };\n\
             \tfunction s($$renderer) {\n\
             \t\tFoo($$renderer, $.spread_props([n]));\n\
             \t}\n\
             \ts($$renderer);\n\
             }\n"
    );
    // The same discipline for a prop and a plain top-level const, and with
    // the component nested inside an element.
    for source in [
        "<script>let { p } = $props();</script>\n{#snippet s()}<Foo {...p} />{/snippet}\n{@render s()}",
        "<script>const c = { a: 1 };</script>\n{#snippet s()}<Foo {...c} />{/snippet}\n{@render s()}",
        "<script>let n = $state({ a: 1 });</script>\n{#snippet s()}<div><Foo {...n} /></div>{/snippet}\n{@render s()}",
    ] {
        let js = compile_js(source);
        assert!(
            js.contains("export default function Input")
                && js.find("function s($$renderer)").unwrap()
                    > js.find("export default function Input").unwrap(),
            "snippet must stay inside the component body for {source:?}:\n{js}"
        );
    }
}

#[test]
fn compile_snippet_component_spread_of_import_still_hoists() {
    // Imports (and globals) don't disqualify hoisting — a component spread of
    // an import keeps the snippet at module scope.
    let js = compile_js(
        "<script>import Foo from './Foo.svelte';\n\timport { cfg } from './cfg.js';</script>\n{#snippet s()}<Foo {...cfg} />{/snippet}\n{@render s()}",
    );
    assert!(
        js.find("function s($$renderer)").unwrap()
            < js.find("export default function Input").unwrap(),
        "import-spread snippet must module-hoist: {js}"
    );
    let js = compile_js(
        "<script>import Foo from './Foo.svelte';</script>\n{#snippet s()}<Foo {...globalThis.cfg} />{/snippet}\n{@render s()}",
    );
    assert!(
        js.find("function s($$renderer)").unwrap()
            < js.find("export default function Input").unwrap(),
        "global-spread snippet must module-hoist: {js}"
    );
}

#[test]
fn compile_render_prop_snippet_is_dynamic() {
    // `{@render children()}` where `children` is a prop is dynamic, so the
    // render tag keeps the trailing `<!---->` even as the sole child.
    let js = compile_js("<script>let { children } = $props();</script>\n{@render children()}");
    assert!(
        js.contains("children($$renderer);\n\t$$renderer.push(`<!---->`);"),
        "dynamic prop render must keep the anchor: {js}"
    );
}

#[test]
fn compile_render_optional_callee() {
    // `{@render foo?.()}` → `foo?.($$renderer)`.
    let js = compile_js("{#snippet foo()}<b>s</b>{/snippet}\n{@render foo?.()}");
    assert!(js.contains("foo?.($$renderer);"), "{js}");
}

#[test]
fn compile_typed_and_generic_snippet() {
    // A `: T` parameter annotation and a `<T>` clause are both ordinary
    // erasure: the oracle emits `function foo($$renderer, x)` either way, the
    // type-level syntax simply gone.
    let js = compile_js(
        "<script lang=\"ts\">\n\tlet { n }: { n: number } = $props();\n</script>\n\
             {#snippet foo(x: number)}<p>{x}</p>{/snippet}\n{@render foo(n)}",
    );
    assert!(
        js.contains("function foo($$renderer, x) {"),
        "annotated snippet param must erase: {js}"
    );
    let generic = compile_js(
        "<script lang=\"ts\">\n\tlet { n }: { n: number } = $props();\n</script>\n\
             {#snippet foo<T>(x: T)}<p>{x}</p>{/snippet}\n{@render foo(n)}",
    );
    assert!(
        generic.contains("function foo($$renderer, x) {"),
        "generic snippet must erase its <T>: {generic}"
    );
}

#[test]
fn compile_rejects_render_member_callee() {
    assert_unsupported(
        "<script>let { obj } = $props();</script>\n{@render obj.snip()}",
        "{@render} callee is not a resolvable local snippet or snippet prop",
    );
}

#[test]
fn compile_rejects_duplicate_snippet_name() {
    assert_unsupported(
        "{#snippet foo()}<b>1</b>{/snippet}\n{#snippet foo()}<b>2</b>{/snippet}\n{@render foo()}",
        "duplicate {#snippet} foo",
    );
}

#[test]
fn compile_rejects_rune_inside_block() {
    // The guard runs on block test / body expressions too.
    assert_unsupported("{#if $state(0)}<p>x</p>{/if}", "$state");
    assert_unsupported(
        "<script>let { items } = $props();</script>\n{#each items as item}<p>{$state(0)}</p>{/each}",
        "$state",
    );
}

#[test]
fn compile_rejects_a_duplicate_snippet_name_in_a_nested_fragment() {
    // The oracle's snippet-name scope is the enclosing FRAGMENT
    // (`phases/scope.js:1335` declares into `state.scope`), not the component, so
    // the rule reaches every fragment — not just the root.
    assert_unsupported(
        "<div>{#snippet a()}x{/snippet}{#snippet a()}y{/snippet}</div>",
        "duplicate {#snippet} a",
    );
    assert_unsupported(
        "{#if x}{#snippet a()}x{/snippet}{#snippet a()}y{/snippet}{/if}",
        "duplicate {#snippet} a",
    );
    assert_unsupported(
        "<svelte:boundary>{#snippet a()}x{/snippet}{#snippet a()}y{/snippet}</svelte:boundary>",
        "duplicate {#snippet} a",
    );
}

#[test]
fn compile_accepts_same_named_snippets_in_different_fragments() {
    // The must-not-over-refuse companion: a fragment is a fresh scope, so the same
    // name one fragment deeper is legal. Live-probed against the oracle.
    //
    // A TOP-LEVEL snippet of the shared name is legal too now that the hoist map is
    // keyed by snippet identity — see
    // `compile_places_a_nested_snippet_named_after_a_top_level_snippet`.
    let _ = compile_js("<div>{#snippet a()}x{/snippet}{#if y}{#snippet a()}z{/snippet}{/if}</div>");
    // A nested snippet whose name matches nothing at the top level.
    let _ = compile_js("{#snippet top()}t{/snippet}<div>{#snippet a()}x{/snippet}</div>");
}

#[test]
fn compile_places_a_nested_snippet_named_after_a_top_level_snippet() {
    // A NESTED `{#snippet}` sharing a top-level snippet's name is legal on both
    // sides — a fragment is a fresh scope, so the oracle places the two
    // declarations independently. The hoist decision is keyed by snippet IDENTITY
    // (`SnippetAnalysis`'s span set), so the nested twin never inherits the
    // top-level one's verdict; both former sub-cases now COMPILE (they were the
    // retired `NestedSnippetNameCollision` refusal). Byte-parity is pinned by the
    // `snippets/nested_name_{hoisted,body}` compile fixtures; these assertions pin
    // the placement directly and cover the fixpoint case (b′) that has no fixture.

    // (a) the top-level `a` HOISTS (static, no instance binding) → module scope;
    // the nested `a` reads the instance binding `v` → the component body. The nested
    // twin does NOT follow the module-hoisted one.
    let hoisted = compile_js(
        "<script>let v = 1;</script>\
         {#snippet a()}static{/snippet}<div>{#snippet a()}{v}{/snippet}</div>{@render a()}",
    );
    let (before_export, after_export) = hoisted
        .split_once("export default")
        .expect("compiled component exports a default");
    assert!(
        before_export.contains("function a("),
        "expected the static `a` to hoist to module scope, got:\n{hoisted}"
    );
    assert!(
        after_export.contains("function a("),
        "expected the state-reading `a` in the component body, got:\n{hoisted}"
    );

    // (b) the top-level `a` reads `v` → the component body; the nested `a` (static)
    // ALSO lands in the body. Two body `function a` declarations, the top-level
    // (direct) one emitted FIRST — the oracle's order, reproduced by
    // `collect_hoisted_snippets`'s recursive-direct-first walk.
    let body = compile_js(
        "<script>let v = 1;</script>\
         <div>{#snippet a()}nested{/snippet}</div>{#snippet a()}{v}{/snippet}{@render a()}",
    );
    let (before_export, after_export) = body
        .split_once("export default")
        .expect("compiled component exports a default");
    assert!(
        !before_export.contains("function a("),
        "expected no module-scope `a`, got:\n{body}"
    );
    assert_eq!(
        after_export.matches("function a(").count(),
        2,
        "expected two body `function a` declarations, got:\n{body}"
    );
    let one = after_export
        .find("`<!---->1`")
        .expect("the top-level `a` body renders `1`");
    let nested = after_export
        .find("`<!---->nested`")
        .expect("the nested `a` body renders `nested`");
    assert!(
        one < nested,
        "expected the top-level (direct) `a` emitted first, got:\n{body}"
    );

    // (b′) the same all-body placement reached through the hoist FIXPOINT: the
    // top-level `a` is itself free of instance bindings but is demoted because the
    // snippet it renders (`b`) reads `v` and cannot hoist. No fixture covers this.
    let fixpoint = compile_js(
        "<script>let v = 1;</script>\
         <div>{#snippet a()}nested{/snippet}</div>\
         {#snippet b()}{v}{/snippet}{#snippet a()}{@render b()}{/snippet}{@render a()}",
    );
    let (before_export, after_export) = fixpoint
        .split_once("export default")
        .expect("compiled component exports a default");
    assert!(
        !before_export.contains("function a("),
        "expected no module-scope `a` (demoted by the fixpoint), got:\n{fixpoint}"
    );
    assert_eq!(
        after_export.matches("function a(").count(),
        2,
        "expected two body `function a` declarations, got:\n{fixpoint}"
    );
}

#[test]
fn compile_rejects_a_top_level_snippet_shadowing_a_script_declaration() {
    // `declaration_duplicate` at `2-analyze/visitors/SnippetBlock.js:34` — the
    // OTHER call site of that oracle error code, snippet-vs-script.
    assert_unsupported(
        "<script>let foo = 'bar';</script>{#snippet foo()}baz{/snippet}",
        "{#snippet} foo is already declared by the instance script",
    );
}

#[test]
fn compile_accepts_a_top_level_snippet_named_after_an_import() {
    // The must-not-over-refuse companion: `Scope.declare` forwards an `import` to
    // the PARENT scope (`phases/scope.js:679-681`), so an instance-script import
    // lands in `module.scope.declarations` and is never in the
    // `instance.scope.declarations` `declaration_duplicate` tests. All four import
    // forms, live-probed — the oracle compiles each and emits the import.
    let _ = compile_js("<script>import C from './C.svelte';</script>{#snippet C()}x{/snippet}");
    let _ = compile_js("<script>import {a} from './x.js';</script>{#snippet a()}x{/snippet}");
    let _ = compile_js("<script>import * as ns from './x.js';</script>{#snippet ns()}x{/snippet}");
    let _ = compile_js("<script>import {a as b} from './x.js';</script>{#snippet b()}x{/snippet}");
    // Same parent-scope argument from the other direction: a MODULE-script
    // declaration is not in `instance.scope.declarations` either.
    let _ = compile_js("<script module>let m = 1;</script>{#snippet m()}x{/snippet}");
}

#[test]
fn compile_rejects_a_top_level_snippet_shadowing_a_non_import_declaration() {
    // The control on the other side of the import carve-out: a `$state` binding IS
    // an instance-scope declaration, so the rule still fires. Live-probed.
    assert_unsupported(
        "<script>let x = $state(1);</script>{#snippet x()}y{/snippet}",
        "{#snippet} x is already declared by the instance script",
    );
}

#[test]
fn compile_accepts_a_nested_snippet_named_after_a_script_declaration() {
    // The rule is `is_top_level` only: a snippet below the root fragment declares
    // into that fragment's scope, never the instance scope. Live-probed.
    let _ = compile_js("<script>let foo = 1;</script><div>{#snippet foo()}b{/snippet}</div>");
}

#[test]
fn snippet_script_duplicate_precedes_module_refusals() {
    // `validate_top_level_snippets` reads the instance binding table and nothing
    // else, so it runs in `analyze()` as soon as that table exists — BEFORE the
    // module↔instance collision check below it. A component tripping both
    // therefore reports this one.
    //
    // Nothing makes that order more oracle-faithful than the reverse:
    // `ModuleInstanceNameCollision` is a tsv over-refusal, not a ported oracle
    // rule, so this is bucket ATTRIBUTION only — no component's accept/reject
    // verdict turns on it. Pinned so a reorder shows up as a failing test rather
    // than as a silent corpus re-bucketing.
    //
    // (Every module-script *analysis* refusal is unaffected either way: they all
    // fire in `analyze_module_script`, which runs long before both.)
    assert_unsupported(
        "<script module>let foo = 1;</script><script>let foo = 2;</script>\
         {#snippet foo()}x{/snippet}",
        "{#snippet} foo is already declared by the instance script",
    );
}

#[test]
fn compile_rejects_a_snippet_shadowing_a_component_prop() {
    // `snippet_shadowing_prop` (`SnippetBlock.js:59`) — a plain attribute, and the
    // `BindDirective` arm beside it.
    assert_unsupported(
        "<C title=\"\">{#snippet title()}t{/snippet}</C>",
        "{#snippet} title shadows the component prop",
    );
    assert_unsupported(
        "<script>let v = $state(1);</script><C bind:title={v}>{#snippet title()}t{/snippet}</C>",
        "{#snippet} title shadows the component prop",
    );
}

#[test]
fn compile_accepts_the_shadowing_prop_rule_s_exemptions() {
    // No same-named attribute, and — the claim that matters — the rule does NOT
    // fire at depth: `path.at(-2)` is the snippet's fragment's parent, so one more
    // level of nesting takes the component out of that slot. Both live-probed.
    let _ = compile_js("<C>{#snippet title()}t{/snippet}</C>");
    let _ = compile_js("<C title=\"\"><div>{#snippet title()}t{/snippet}</div></C>");
}

#[test]
fn compile_rejects_a_children_snippet_beside_other_content() {
    // `snippet_conflict` (`SnippetBlock.js:77`).
    assert_unsupported(
        "<Foo>hello{#snippet children()}hi{/snippet}</Foo>",
        "{#snippet children()} alongside other default content",
    );
}

#[test]
fn compile_accepts_the_children_conflict_rule_s_exemptions() {
    // Every exemption in the oracle's `some(…)` predicate, plus the two gates
    // ahead of it — all live-probed:
    //   whitespace-only text, a comment, a non-`children` name, a non-component
    //   parent.
    let _ = compile_js("<Foo> {#snippet children()}hi{/snippet}</Foo>");
    let _ = compile_js("<Foo><!--c-->{#snippet children()}hi{/snippet}</Foo>");
    let _ = compile_js("<Foo>hello{#snippet other()}hi{/snippet}</Foo>");
    let _ = compile_js("<div>hello{#snippet children()}hi{/snippet}</div>");
}

#[test]
fn compile_non_hoistable_snippets_emit_in_recursive_direct_first_order() {
    // Non-module-hoistable snippets bubbling to the same block scope emit in
    // recursive-direct-first order, NOT document pre-order: a fragment's own
    // direct-child snippets (source order) precede its descendant elements'
    // snippets. Mirrors the oracle's push-order timing
    // (Fragment.js:35-44 + RegularElement.js:229-231).
    //
    // `a` is a root snippet reading instance state `v` (not module-hoistable →
    // component body); `b` is nested in a `<div>` (never hoistable → bubbles to
    // the component body). Document pre-order would emit `b` first (the bug);
    // the oracle emits `a` before `b`.
    let js = compile_js(
        "<script>let v = 1;</script>\n<div>{#snippet b()}nested{/snippet}</div>\n{#snippet a()}{v}{/snippet}\n{@render a()}",
    );
    let a = js.find("function a($$renderer)").unwrap();
    let b = js.find("function b($$renderer)").unwrap();
    assert!(a < b, "expected `function a` before `function b`:\n{js}");

    // Interleaved: a root snippet, then a `<div>` holding one, then another root
    // snippet — the two root snippets (a1, a2) precede the element's (b), and
    // a1 before a2 in source order.
    let js = compile_js(
        "<script>let v = 1;</script>\n{#snippet a1()}{v}{/snippet}\n<div>{#snippet b()}x{/snippet}</div>\n{#snippet a2()}{v}{/snippet}\n{@render a1()}",
    );
    let a1 = js.find("function a1($$renderer)").unwrap();
    let a2 = js.find("function a2($$renderer)").unwrap();
    let b = js.find("function b($$renderer)").unwrap();
    assert!(a1 < a2 && a2 < b, "expected order a1 < a2 < b:\n{js}");
}
