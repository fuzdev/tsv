//! The `style:` directive and the fused `$.attr_style(...)` call.

use super::support::*;

#[test]
fn compile_class_and_style_directive_coexist() {
    // `class:` and `style:` on one element both emit their own fused call — the
    // synthetic-`class` `$.attr_class` before the synthetic-`style` `$.attr_style`
    // (the oracle appends the synthetic empty `class` then the synthetic `style`).
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div class:a={x} style:color={w}>t</div>",
    );
    assert!(
        js.contains(
            "`<div${$.attr_class('', void 0, { a: x })}${$.attr_style('', { color: w })}>t</div>`"
        ),
        "{js}"
    );
}

#[test]
fn compile_style_directive_basic() {
    // A `style:` directive on a regular element fuses with the authored `style`
    // attribute into `$.attr_style(base, { name: value })` — TWO args, no css-hash
    // (style is never scoped).
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div style=\"x\" style:color={w}>text</div>",
    );
    assert!(
        js.contains("`<div${$.attr_style('x', { color: w })}>text</div>`"),
        "{js}"
    );
}

#[test]
fn compile_style_directive_synthetic_and_shorthand() {
    // No authored `style`: the synthetic empty `''` base, emitted after all plain
    // attributes. A shorthand `style:color` prints as object-shorthand `{ color }`
    // (the oracle's `b.id(name)` value coincides with the lowercased key).
    let js = compile_js("<script>let color = $state(1);</script>\n<div style:color>x</div>");
    assert!(
        js.contains("`<div${$.attr_style('', { color })}>x</div>`"),
        "{js}"
    );
}

#[test]
fn compile_style_directive_important_partition() {
    // Any `|important` directive → the 2-element `[ {normal}, {important} ]` array,
    // source order preserved within each group.
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div style:a={w} style:b|important={x} style:c={w}>t</div>",
    );
    assert!(
        js.contains("$.attr_style('', [{ a: w, c: w }, { b: x }])"),
        "{js}"
    );
    // All important → the normal object is empty `{}`.
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div style:a|important={w}>t</div>",
    );
    assert!(js.contains("$.attr_style('', [{}, { a: w }])"), "{js}");
}

#[test]
fn compile_style_directive_key_lowercasing_and_quoting() {
    // A hyphenated / custom property is a quoted string key; a `--custom` key keeps
    // its case, a plain name lowercases.
    let js = compile_js(
        "<script>let w = $state(1);</script>\n<div style:font-weight={w} style:--MyVar={w}>t</div>",
    );
    assert!(
        js.contains("$.attr_style('', { 'font-weight': w, '--MyVar': w })"),
        "{js}"
    );
}

#[test]
fn compile_style_directive_dynamic_base_no_clsx() {
    // A dynamic `style={expr}` base is the BARE expression — no `$.clsx` (unlike
    // `class`).
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div style={w} style:color={x}>t</div>",
    );
    assert!(js.contains("$.attr_style(w, { color: x })"), "{js}");
}

#[test]
fn compile_style_directive_invalid_modifier_refuses() {
    // Only a single `|important` is a legal modifier — any other modifier, or two
    // or more, is `style_directive_invalid_modifier` (an oracle error).
    assert_unsupported(
        "<script>let x = $state(true);</script>\n<div style:color|foo={x}>t</div>",
        "style: directive with an invalid modifier",
    );
    assert_unsupported(
        "<script>let x = $state(true);</script>\n<div style:color|important|bar={x}>t</div>",
        "style: directive with an invalid modifier",
    );
}

#[test]
fn compile_style_directive_mixed_value_refuses() {
    // A mixed-value `style:color="a {b}"` value (text + expression) is deferred.
    assert_unsupported(
        "<script>let b = $state(1);</script>\n<div style:color=\"a {b}\">t</div>",
        "style: directive with a mixed-value (text + expression) value",
    );
}

#[test]
fn compile_style_directive_mixed_base_refuses() {
    // A `style:` directive alongside a mixed-value `style="a {b}"` base is deferred.
    assert_unsupported(
        "<script>let a = $state(1); let x = $state(true);</script>\n<div style=\"a {a}\" style:color={x}>t</div>",
        "style: directive alongside a mixed-value style attribute",
    );
}
