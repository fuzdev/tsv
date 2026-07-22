//! The `class:` directive and the fused `$.attr_class(...)` call.

use super::support::*;

#[test]
fn compile_class_clsx_rule() {
    // The oracle's needs_clsx rule (oracle-probed): only a BARE
    // `class={expr}` wraps in $.clsx, and only when the expression is not
    // a Literal, TemplateLiteral, or ESTree BinaryExpression — logical
    // operators are LogicalExpression there and DO wrap. The quoted form
    // `class="{expr}"` is a one-chunk array in the oracle's AST and NEVER
    // wraps. (Quoted shapes live here, not in a fixture — prettier strips
    // the redundant quotes from fixture inputs.)
    let wraps = |src: &str| compile_js(src).contains("$.clsx(");
    // Bare: identifier / conditional / logical / object / array wrap.
    assert!(wraps(
        "<script>let a = `f`;</script>\n<div class={a}></div>"
    ));
    assert!(wraps(
        "<script>let { x } = $props();</script>\n<div class={x ? `a` : `b`}></div>"
    ));
    assert!(wraps(
        "<script>let { x } = $props();</script>\n<div class={x ?? `a`}></div>"
    ));
    assert!(wraps(
        "<script>let { x } = $props();</script>\n<div class={{ active: x }}></div>"
    ));
    assert!(wraps(
        "<script>let { x } = $props();</script>\n<div class={[x, `b`]}></div>"
    ));
    // Bare exclusions: template literal / arithmetic binary / number literal.
    assert!(!wraps(
        "<script>let { x } = $props();</script>\n<div class={`a ${x}`}></div>"
    ));
    assert!(!wraps(
        "<script>let { x } = $props();</script>\n<div class={x + ` y`}></div>"
    ));
    assert!(!wraps("<div class={5}></div>"));
    // Quoted: never wraps, regardless of expression shape.
    assert!(!wraps(
        "<script>let a = `f`;</script>\n<div class=\"{a}\"></div>"
    ));
    assert!(!wraps(
        "<script>let { x } = $props();</script>\n<div class=\"{{ active: x }}\"></div>"
    ));
    // Non-class dynamic attributes never wrap.
    assert!(!wraps(
        "<script>let a = `f`;</script>\n<div title={a}></div>"
    ));
}

#[test]
fn compile_class_directive_basic() {
    // A `class:` directive on a regular element fuses with the authored `class`
    // attribute into `$.attr_class(base, void 0, { name: expr })` (the oracle's
    // `build_attr_class`). The directive key is a (canonicalized) identifier and
    // the value is the borrowed expression.
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div class=\"foo\" class:active={x}>text</div>",
    );
    assert!(
        js.contains("`<div${$.attr_class('foo', void 0, { active: x })}>text</div>`"),
        "{js}"
    );
}

#[test]
fn compile_class_directive_synthetic_and_shorthand() {
    // No authored `class`: the synthetic empty `''` base, and the fused call
    // emits after all plain attributes. A shorthand `class:active` carries the
    // auto-generated identifier as its value (`{ active: active }`, not collapsed).
    let js = compile_js("<script>let active = $state(true);</script>\n<div class:active>x</div>");
    assert!(
        js.contains("`<div${$.attr_class('', void 0, { active: active })}>x</div>`"),
        "{js}"
    );
}

#[test]
fn compile_class_directive_ordering() {
    // Plain attributes emit inline in source order; the synthetic-`class` fused
    // call emits at the END (after `id` and `title`).
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div id=\"a\" class:x={x} title=\"b\">t</div>",
    );
    assert!(
        js.contains("`<div id=\"a\" title=\"b\"${$.attr_class('', void 0, { x: x })}>t</div>`"),
        "{js}"
    );
    // An authored `class` after the directive: the fused call takes the `class`
    // slot (before the later `id`).
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div class:x={x} class=\"c\" id=\"a\">t</div>",
    );
    assert!(
        js.contains("`<div${$.attr_class('c', void 0, { x: x })} id=\"a\">t</div>`"),
        "{js}"
    );
}

#[test]
fn compile_class_directive_scoping() {
    // Scoped via a static-class token: the hash concatenates into the string base.
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div class=\"foo\" class:active={x}>t</div>\n<style>.foo { color: red }</style>",
    );
    assert!(
        js.contains("$.attr_class('foo svelte-tsvhash', void 0, { active: x })"),
        "static-token scope concat: {js}"
    );
    // Scoped via the directive NAME: the empty base concatenates to just the hash.
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div class:active={x}>t</div>\n<style>.active { color: red }</style>",
    );
    assert!(
        js.contains("$.attr_class('svelte-tsvhash', void 0, { active: x })"),
        "directive-name scope: {js}"
    );
    // Scoped with a dynamic base: the hash rides the 2nd argument.
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div class={w} class:foo={x}>t</div>\n<style>.foo { color: red }</style>",
    );
    assert!(
        js.contains("$.attr_class($.clsx(w), 'svelte-tsvhash', { foo: x })"),
        "dynamic-base scope: {js}"
    );
}

#[test]
fn compile_class_directive_mixed_class_refuses() {
    // A `class:` directive alongside a mixed-value `class="a {b}"` attribute is
    // deferred — the oracle passes the mixed value to `build_attr_class` as the
    // base, a shape this slice does not build.
    assert_unsupported(
        "<script>let a = $state(1); let x = $state(true);</script>\n<div class=\"a {a}\" class:active={x}>t</div>",
        "class: directive alongside a mixed-value class attribute",
    );
}
