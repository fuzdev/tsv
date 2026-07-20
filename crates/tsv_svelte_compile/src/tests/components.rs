//! Component invocation: props, children snippets, anchors, refusals.

use super::support::*;

#[test]
fn compile_self_closing_component() {
    // A plain component invocation compiles to `Name($$renderer, {})`. As the
    // sole root child it is standalone — no trailing `<!---->` anchor.
    let js = compile_js("<Foo />");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tFoo($$renderer, {});\n\
             }\n"
    );
}

#[test]
fn compile_component_prop_value_shapes() {
    // string → 's'; expr(prop) → the reference; shorthand `{value}` collapses
    // to `value`; boolean → `true`. The component declares props, so `$$props`
    // is injected, but no `$$renderer.component` wrapper (a bare prop
    // reference is not `needs_context`-unsafe).
    let js = compile_js(
        "<script>let { x, value } = $props();</script>\n<Foo a=\"s\" b={x} {value} disabled />",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { x, value } = $$props;\n\
             \tFoo($$renderer, { a: 's', b: x, value, disabled: true });\n\
             }\n"
    );
}

#[test]
fn compile_component_shorthand_collapses_when_names_match() {
    // `b={b}` → `{ b }` (key === value identifier); `b={x}` → `{ b: x }`.
    let js = compile_js("<script>let { b } = $props();</script>\n<Foo b={b} />");
    assert!(js.contains("Foo($$renderer, { b });"), "{js}");
    let js = compile_js("<script>let { b } = $props();</script>\n<Foo a={b} />");
    assert!(js.contains("Foo($$renderer, { a: b });"), "{js}");
}

#[test]
fn compile_component_derived_prop_reads_as_call() {
    // A bare `$derived` read in a prop value becomes `d()` — so a `{d}`
    // shorthand is NOT collapsed (the value is a call, not the identifier).
    let js = compile_js(
        "<script>let n = $state(1);\n\tlet d = $derived(n * 2);\n\tfunction inc() {\n\t\tn++;\n\t}</script>\n<Foo a={d} {d} />",
    );
    assert!(js.contains("Foo($$renderer, { a: d(), d: d() });"), "{js}");
}

#[test]
fn compile_component_mixed_and_string_value_semantics() {
    // Mixed text+expr → a template literal with `$.stringify`; a single static
    // text value entity-decodes but is NOT HTML-escaped (a JS value, not
    // markup); an all-fold mixed value collapses to a string literal.
    let js = compile_js("<script>let { y } = $props();</script>\n<Foo a=\"x {y} z\" />");
    assert!(
        js.contains("Foo($$renderer, { a: `x ${$.stringify(y)} z` });"),
        "{js}"
    );
    let js = compile_js("<Foo a=\"&amp; &lt; &gt;\" />");
    assert!(js.contains("Foo($$renderer, { a: '& < >' });"), "{js}");
    let js = compile_js("<script>let a = 1;\n\tlet b = 2;</script>\n<Foo t=\"x{a}y{b}\" />");
    assert!(js.contains("Foo($$renderer, { t: 'x1y2' });"), "{js}");
}

#[test]
fn compile_component_non_identifier_key_quotes() {
    let js = compile_js("<Foo data-x=\"1\" aria-label=\"hi\" />");
    assert!(
        js.contains("Foo($$renderer, { 'data-x': '1', 'aria-label': 'hi' });"),
        "{js}"
    );
}

#[test]
fn compile_component_spread_props() {
    // Consecutive props group into object literals; spreads break the run,
    // wrapping the whole thing in `$.spread_props([...])`.
    let js = compile_js("<script>let { r } = $props();</script>\n<Foo a={1} {...r} b={2} />");
    assert!(
        js.contains("Foo($$renderer, $.spread_props([{ a: 1 }, r, { b: 2 }]));"),
        "{js}"
    );
    let js = compile_js("<script>let { r, s } = $props();</script>\n<Foo {...r} {...s} />");
    assert!(
        js.contains("Foo($$renderer, $.spread_props([r, s]));"),
        "{js}"
    );
}

#[test]
fn compile_component_event_handler_is_a_plain_prop() {
    // Unlike an element `on*` handler (dropped), a component `onclick={fn}` is
    // an ordinary prop.
    let js = compile_js("<script>function fn() {}</script>\n<Foo onclick={fn} />");
    assert!(js.contains("Foo($$renderer, { onclick: fn });"), "{js}");
}

#[test]
fn compile_component_anchor_when_not_standalone() {
    // Inside an element the component is not standalone → trailing `<!---->`.
    let js = compile_js("<div><Foo /></div>");
    assert!(
        js.contains("$$renderer.push(`<div>`);")
            && js.contains("Foo($$renderer, {});")
            && js.contains("$$renderer.push(`<!----></div>`);"),
        "{js}"
    );
    // Two sibling components each get an anchor (not a sole child).
    let js = compile_js("<Foo /><Bar />");
    assert!(
        js.contains("Foo($$renderer, {});")
            && js.contains("$$renderer.push(`<!---->`);")
            && js.contains("Bar($$renderer, {});"),
        "{js}"
    );
}

#[test]
fn compile_component_sole_block_child_is_standalone() {
    // `{#if a}<Foo/>{/if}` — the component is the branch's sole child, so it
    // reuses the branch anchor and emits no trailing `<!---->`.
    let js = compile_js("{#if a}<Foo />{/if}");
    assert!(js.contains("Foo($$renderer, {});"), "{js}");
    assert!(
        !js.contains("$$renderer.push(`<!---->`)"),
        "sole block-child component must not add an anchor: {js}"
    );
}

#[test]
fn compile_refuses_dynamic_components() {
    // A member component and a component named after a reactive binding
    // (prop / $state / $derived / each-local) all compile to the oracle's
    // truthiness guard — refused in this slice.
    assert_unsupported("<Foo.Bar />", "dynamic <Foo.Bar> component");
    assert_unsupported(
        "<script>let { Foo } = $props();</script>\n<Foo />",
        "dynamic <Foo> component",
    );
    assert_unsupported(
        "<script>let Foo = $state(null);</script>\n<Foo />",
        "dynamic <Foo> component",
    );
    assert_unsupported(
        "<script>let n = $state(1);\n\tlet Foo = $derived(n);\n\tfunction f() {\n\t\tn++;\n\t}</script>\n<Foo />",
        "dynamic <Foo> component",
    );
    // A plain local / import is NOT dynamic — it compiles.
    compile_checked("<script>const Foo = null;</script>\n<Foo />");
}

#[test]
fn compile_component_children_snippet_prop() {
    // Default-slot children compile to a `children: ($$renderer) => {…}`
    // snippet prop plus `$$slots: { default: true }`. A text-first body gets
    // the `<!---->` marker.
    let js = compile_js("<Foo><p>hi</p></Foo>");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tFoo($$renderer, {\n\
             \t\tchildren: ($$renderer) => {\n\
             \t\t\t$$renderer.push(`<p>hi</p>`);\n\
             \t\t},\n\
             \t\t$$slots: { default: true }\n\
             \t});\n\
             }\n"
    );
    // Text-first children get the `<!---->` anchor inside the arrow.
    let js = compile_js("<Foo>hi <b>x</b></Foo>");
    assert!(
        js.contains("$$renderer.push(`<!---->hi <b>x</b>`);"),
        "{js}"
    );
    // An empty / whitespace-only body is NOT children (no `children` prop).
    let js = compile_js("<Foo></Foo>");
    assert_eq!(js.matches("children").count(), 0, "{js}");
    let js = compile_js("<Foo>   </Foo>");
    assert_eq!(js.matches("children").count(), 0, "{js}");
}

#[test]
fn compile_component_children_after_attrs_and_spread() {
    // The `children` prop appends after attribute props.
    let js = compile_js("<Foo a=\"x\"><p>hi</p></Foo>");
    assert!(
        js.contains("a: 'x'") && js.contains("children: ($$renderer) =>"),
        "{js}"
    );
    // With a trailing spread the children go to their own object element.
    let js = compile_js("<script>let { r } = $props();</script>\n<Foo {...r}><p>hi</p></Foo>");
    assert!(js.contains("$.spread_props(["), "{js}");
    assert!(js.contains("children: ($$renderer) =>"), "{js}");
    assert!(js.contains("$$slots: { default: true }"), "{js}");
}

#[test]
fn compile_component_named_snippet_props() {
    // A `{#snippet}` child compiles to a `function` in a wrapping block plus a
    // `{ name }` shorthand prop and a `$$slots: { name: true }` entry.
    let js = compile_js("<Foo>{#snippet header()}<h1>t</h1>{/snippet}</Foo>");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \t{\n\
             \t\tfunction header($$renderer) {\n\
             \t\t\t$$renderer.push(`<h1>t</h1>`);\n\
             \t\t}\n\
             \t\tFoo($$renderer, { header, $$slots: { header: true } });\n\
             \t}\n\
             }\n"
    );
    // Multiple snippets: functions and slot entries in source order.
    let js =
        compile_js("<Foo>{#snippet a()}<b>1</b>{/snippet}{#snippet b()}<i>2</i>{/snippet}</Foo>");
    assert!(
        js.contains("Foo($$renderer, { a, b, $$slots: { a: true, b: true } });"),
        "{js}"
    );
    // A snippet named `children` keeps the `children` prop but a `default`
    // slot key.
    let js = compile_js("<Foo>{#snippet children()}<p>c</p>{/snippet}</Foo>");
    assert!(
        js.contains("Foo($$renderer, { children, $$slots: { default: true } });"),
        "{js}"
    );
}

#[test]
fn compile_component_snippet_and_default_children() {
    // Mixed named snippet + default children: the `children` arrow holds only
    // the default children (the snippet is in the wrapping block), and
    // `$$slots` carries both keys.
    let js = compile_js("<Foo>text{#snippet header()}<h1>t</h1>{/snippet}</Foo>");
    assert!(js.contains("function header($$renderer) {"), "{js}");
    assert!(js.contains("header,"), "{js}");
    assert!(js.contains("children: ($$renderer) =>"), "{js}");
    assert!(js.contains("$$renderer.push(`<!---->text`);"), "{js}");
    assert!(
        js.contains("$$slots: { header: true, default: true }"),
        "{js}"
    );
}

#[test]
fn compile_refuses_deferred_component_children() {
    // A `slot="…"` child (named slot) is a later slice; an explicit `children`
    // prop + default children is the oracle's `$$slots.default` divergence.
    assert_unsupported(
        "<Foo><p slot=\"header\">hi</p></Foo>",
        "named slot on <Foo> component",
    );
    assert_unsupported(
        "<script>let { c } = $props();</script>\n<Foo children={c}><p>hi</p></Foo>",
        "both a children prop and default children",
    );
}

#[test]
fn compile_refuses_component_directives_and_css_vars() {
    // `--custom-property` → `$.css_props`; `bind:` → a settle loop; other
    // directives are (mostly) oracle-rejected — all refused here.
    assert_unsupported(
        "<Foo --my-color=\"red\" />",
        "--custom-property attribute on <Foo> component",
    );
    assert_unsupported(
        "<script>let { v } = $props();</script>\n<Foo bind:value={v} />",
        "bind: directive on <Foo> component",
    );
}

#[test]
fn compile_carries_comments_with_component() {
    // Carried script comments alongside a component invocation carry through: the
    // component call's prop values are template-region borrows, so the comment
    // stays a leading comment of its script statement.
    let js = compile_js("<script>\n\t// note\n\tlet x = 1;\n</script>\n<Foo a={x} />");
    assert!(
        js.contains("// note"),
        "the script comment must carry through: {js}"
    );
}

#[test]
fn compile_component_prop_new_expression_wraps() {
    // A `new` in a prop value drives `needs_context` (walked in
    // needs_context.rs), wrapping the body and injecting `$$props`.
    let js = compile_js("<Foo a={new Date()} />");
    assert!(
        js.contains("$$renderer.component(($$renderer) =>")
            && js.contains("Foo($$renderer, { a: new Date() });"),
        "{js}"
    );
}

#[test]
fn compile_component_spread_member_on_prop_wraps() {
    // A member access inside a component spread must feed needs_context.
    let js = compile_js("<script>let { p } = $props();</script>\n<Foo {...p.x} />");
    assert!(
        js.contains("$$renderer.component(($$renderer) =>"),
        "spread member-on-prop must wrap: {js}"
    );
}

#[test]
fn compile_refuses_const_tag_shadowing_derived() {
    // A `{@const}` that shadows a top-level `$derived` refuses (the
    // name-based derived-read rewrite would wrongly call the const as `d()`).
    assert_unsupported(
        "<script>\n\tlet a = $state(1);\n\tlet d = $derived(a * 2);\n\tlet { items } = $props();\n\tfunction f() {\n\t\ta++;\n\t}\n</script>\n{#each items as item}{@const d = item.x}<p>{d}</p>{/each}",
        "shadows a $derived binding",
    );
}
