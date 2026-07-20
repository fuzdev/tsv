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
