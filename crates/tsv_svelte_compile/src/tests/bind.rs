//! The `bind:` directives: the supported core kinds and every refusal.

use super::support::*;
use crate::*;

#[test]
fn compile_bind_this_omits() {
    // `bind:this` is omitted on any regular element (the oracle's early
    // `continue`) and works for any variable — no `$state` gate, nothing emitted.
    let js = compile_js("<script>let el = $state();</script>\n<div bind:this={el}>t</div>");
    assert!(js.contains("`<div>t</div>`"), "{js}");
}

#[test]
fn compile_bind_value_and_member_emit_attr() {
    // `bind:value` on `<input>` → `$.attr('value', expr)`; a member target rides
    // through (`obj.x`), a dynamic `type={x}` is fine for `value`.
    let js = compile_js("<script>let v = $state('');</script>\n<input bind:value={v}>");
    assert!(js.contains("$.attr('value', v)"), "{js}");
    let js = compile_js("<script>let obj = $state({ x: 1 });</script>\n<input bind:value={obj.x}>");
    assert!(js.contains("$.attr('value', obj.x)"), "{js}");
    let js = compile_js(
        "<script>let v = $state(''); let t = $state('text');</script>\n<input type={t} bind:value={v}>",
    );
    assert!(js.contains("$.attr('value', v)"), "{js}");
}

#[test]
fn compile_bind_checked_checkbox_emits_boolean_attr() {
    // `bind:checked` on a static `type="checkbox"` → `$.attr('checked', c, true)`.
    let js = compile_js(
        "<script>let c = $state(false);</script>\n<input type=\"checkbox\" bind:checked={c}>",
    );
    assert!(js.contains("$.attr('checked', c, true)"), "{js}");
}

#[test]
fn compile_bind_group_synthesizes_checked() {
    // `bind:group` synthesizes a `checked`: `group === value` (radio/other static
    // type) or `group.includes(value)` (checkbox). The companion `value` attribute
    // still emits at its own slot.
    let js = compile_js(
        "<script>let g = $state('a');</script>\n<input type=\"radio\" bind:group={g} value=\"a\">",
    );
    assert!(js.contains("$.attr('checked', g === 'a', true)"), "{js}");
    assert!(
        js.contains(" value=\"a\""),
        "companion value still emits: {js}"
    );
    let js = compile_js(
        "<script>let g = $state('a');</script>\n<input type=\"checkbox\" bind:group={g} value=\"a\">",
    );
    assert!(
        js.contains("$.attr('checked', g.includes('a'), true)"),
        "{js}"
    );
    // A dynamic companion `value={x}`: the synthesis reads `x` AND `value={x}`
    // still emits its own `$.attr('value', x)`.
    let js = compile_js(
        "<script>let g = $state('a'); let x = $state(1);</script>\n<input type=\"checkbox\" bind:group={g} value={x}>",
    );
    assert!(
        js.contains("$.attr('checked', g.includes(x), true)"),
        "{js}"
    );
    assert!(js.contains("$.attr('value', x)"), "{js}");
}

#[test]
fn compile_bind_group_no_companion_value_drops() {
    // No companion `value` attribute → the oracle silently drops the group bind.
    let js =
        compile_js("<script>let g = $state('a');</script>\n<input type=\"radio\" bind:group={g}>");
    assert!(js.contains("`<input type=\"radio\"/>`"), "{js}");
}

#[test]
fn compile_bind_coexists_with_class_directive() {
    // `bind:value` (inline) and `class:x` (pre-scanned, fused, synthetic slot) both
    // emit — the value attr first, then the fused class call after all plain attrs.
    let js = compile_js(
        "<script>let v = $state(''); let c = $state(false);</script>\n<input bind:value={v} class:x={c}>",
    );
    assert!(
        js.contains("$.attr('value', v)}${$.attr_class('', void 0, { x: c })}"),
        "{js}"
    );
}

#[test]
fn compile_bind_invalid_target_refuses() {
    // A `value`/`checked` bind on a non-`<input>` element, or `value` on
    // `<textarea>` — the oracle rejects the target (or the shape is unimplemented).
    assert_unsupported(
        "<script>let v = $state('');</script>\n<div bind:value={v}></div>",
        "bind: directive value",
    );
    assert_unsupported(
        "<script>let v = $state('');</script>\n<textarea bind:value={v}></textarea>",
        "bind: directive value",
    );
}

#[test]
fn compile_bind_checked_requires_static_checkbox_type() {
    // `bind:checked` requires a static `type="checkbox"` — a missing / non-checkbox
    // type is `bind_invalid_target` (an oracle error).
    assert_unsupported(
        "<script>let c = $state(false);</script>\n<input bind:checked={c}>",
        "bind: directive checked",
    );
    assert_unsupported(
        "<script>let c = $state(false);</script>\n<input type=\"radio\" bind:checked={c}>",
        "bind: directive checked",
    );
}

#[test]
fn compile_bind_group_dynamic_type_refuses() {
    // A dynamic `type={x}` with `bind:group` is `attribute_invalid_type` (an oracle
    // error) — refuse rather than over-accept.
    assert_unsupported(
        "<script>let g = $state('a'); let t = $state('radio');</script>\n<input type={t} bind:group={g} value=\"a\">",
        "bind: directive group",
    );
}

#[test]
fn compile_bind_value_bare_type_and_file_refuse() {
    // A BARE `type` with `bind:value` is `attribute_invalid_type` (an oracle error);
    // a static `type="file"` is the files trap the oracle silently drops the bind
    // for — refuse rather than emit a divergent `$.attr('value', …)`.
    assert_unsupported(
        "<script>let v = $state('');</script>\n<input type bind:value={v}>",
        "bind: directive value",
    );
    assert_unsupported(
        "<script>let v = $state('');</script>\n<input type=\"file\" bind:value={v}>",
        "bind: directive value",
    );
}

#[test]
fn compile_bind_omit_in_ssr_and_special_targets_refuse() {
    // The `omit_in_ssr` media/dimension binds, `bind:open` on `<details>`, and the
    // content-editable trio are all deferred → the collapsing `bind:` bucket.
    assert_unsupported(
        "<script>let w = $state(0);</script>\n<div bind:clientWidth={w}></div>",
        "bind: directive clientWidth",
    );
    assert_unsupported(
        "<script>let o = $state(false);</script>\n<details bind:open={o}></details>",
        "bind: directive open",
    );
    assert_unsupported(
        "<script>let h = $state('');</script>\n<div contenteditable bind:innerHTML={h}></div>",
        "bind: directive innerHTML",
    );
}

#[test]
fn compile_bind_non_state_expression_refuses() {
    // The expression-validity gate: a non-lvalue target (a call) and a bind rooted
    // at a non-`$state` binding (a `$derived`) both refuse — tsv emits only a
    // `$state`-rooted lvalue (the SAFE side of the oracle's assignable rule).
    assert_unsupported(
        "<script>let f = () => '';</script>\n<input bind:value={f()}>",
        "bind: directive value",
    );
    assert_unsupported(
        "<script>let n = $state(1); let d = $derived(n + 1);</script>\n<input bind:value={d}>",
        "bind: directive value",
    );
}

#[test]
fn compile_bind_this_non_lvalue_refuses() {
    // `bind:this` binds any variable (no `$state` gate), but the target must still
    // be an assignable lvalue — an Identifier or member chain. A non-lvalue target
    // (call, literal, logical) is the oracle's `bind_invalid_expression`; refuse
    // rather than silently omit the bind.
    assert_unsupported(
        "<script>let f = () => '';</script>\n<div bind:this={f()}></div>",
        "bind: directive this",
    );
    assert_unsupported("<div bind:this={42}></div>", "bind: directive this");
    assert_unsupported("<div bind:this={a && b}></div>", "bind: directive this");
    // A plain `let` (no `$state`) is a valid `bind:this` target and still omits, as
    // does a member-chain lvalue — both compile with no `this` attribute.
    let js = compile_js("<script>let el;</script>\n<div bind:this={el}>t</div>");
    assert!(js.contains("`<div>t</div>`"), "{js}");
    let js = compile_js("<script>let obj = {};</script>\n<div bind:this={obj.x}>t</div>");
    assert!(js.contains("`<div>t</div>`"), "{js}");
    // A `{get, set}` pair (the oracle's third valid bind form) also omits in SSR —
    // it is not an lvalue but is a legal bind target, so refuse-don't-omit would
    // over-refuse a valid component (the corpus's `bind-getter-setter-loop`).
    let js =
        compile_js("<script>let el;</script>\n<div bind:this={() => el, (v) => (el = v)}>t</div>");
    assert!(js.contains("`<div>t</div>`"), "{js}");
}

#[test]
fn compile_const_bind_target_refuses() {
    // The oracle's `constant_binding` rejects a bind whose target IDENTIFIER is
    // `const`-declared or an import — keyed on the declaration keyword, so a
    // reactive `const c = $state(0)` is refused too. Corpus-invisible (idiomatic
    // bindables are `let`), so these tests are the only guard.
    assert_unsupported(
        "<script>const c = $state('');</script><input bind:value={c} />",
        "a constant",
    );
    assert_unsupported(
        "<script>const c = $state.raw('');</script><input bind:value={c} />",
        "a constant",
    );
    assert_unsupported(
        "<script>const c = $state(false);</script><input type=\"checkbox\" bind:checked={c} />",
        "a constant",
    );
    // `bind:this` takes any lvalue with no `$state` gate, so it needs the same
    // guard — including for a plain (non-rune) const and an import.
    assert_unsupported(
        "<script>const el = $state(null);</script><div bind:this={el}></div>",
        "a constant",
    );
    assert_unsupported(
        "<script>const el = null;</script><div bind:this={el}></div>",
        "a constant",
    );
    assert_unsupported(
        "<script>import {thing} from './m.js';</script><div bind:this={thing}></div>",
        "a constant",
    );
    // The SSR-inert special-element path shares the primitive.
    assert_unsupported(
        "<script>const w = $state(0);</script><svelte:window bind:innerWidth={w} />",
        "a constant",
    );
    // A MODULE-script const or import is unassignable on exactly the same terms —
    // the rule reads the declaration keyword, not which script declared it. Only
    // `bind:this` exposes this (the `$state`-gated binds refuse a module binding
    // anyway, since it never reaches `state_names`), and it exposes it on all
    // three bind paths.
    assert_unsupported(
        "<script module>const el = null;</script><div bind:this={el}></div>",
        "a constant",
    );
    assert_unsupported(
        "<script module>import {thing} from './m.js';</script><div bind:this={thing}></div>",
        "a constant",
    );
    assert_unsupported(
        "<script module>const el = null;</script>\
         <svelte:element this=\"div\" bind:this={el}></svelte:element>",
        "a constant",
    );
    assert_unsupported(
        "<script module>const el = null;</script><svelte:window bind:this={el} />",
        "a constant",
    );
}

#[test]
fn compile_const_bind_target_through_member_is_allowed() {
    // The oracle refuses only REBINDING a const name. Writing THROUGH one mutates
    // the object, so a member-chain target rooted at a const is accepted — its
    // `validate_no_const_assignment` tests `Identifier` and lets a
    // `MemberExpression` fall through. Walking the chain to its root here would
    // over-refuse a common shape, so this pins the boundary.
    for source in [
        "<script>const obj = $state({v: ''});</script><input bind:value={obj.v} />",
        "<script>const obj = $state({el: null});</script><div bind:this={obj.el}></div>",
        // A plain `let` target is untouched by the const gate.
        "<script>let c = $state('');</script><input bind:value={c} />",
        "<script>let el = $state(null);</script><div bind:this={el}></div>",
        // Same boundary for a MODULE-script const — the member chain writes
        // through it, so the oracle accepts and the widened set must not refuse.
        "<script module>const obj = {el: null};</script><div bind:this={obj.el}></div>",
        // A top-level class or function is `declaration_kind` 'class'/'function',
        // never 'const', so the oracle's test does not fire — in either script.
        "<script module>class C {}</script><div bind:this={C}></div>",
        "<script module>function f() {}</script><div bind:this={f}></div>",
        "<script>class C {}</script><div bind:this={C}></div>",
        "<script>function f() {}</script><div bind:this={f}></div>",
    ] {
        compile(source, &CompileOptions::default())
            .unwrap_or_else(|err| panic!("must still compile: {err:?} for:\n{source}"));
    }
}

#[test]
fn compile_optional_chain_bind_target_refuses() {
    // acorn wraps a chain containing an optional link in a `ChainExpression`, and
    // the oracle's bind-expression test admits only `Identifier` /
    // `MemberExpression` / a `{get, set}` pair — so `bind:this={o?.el}` is
    // `bind_invalid_expression`. tsv has no chain wrapper, so `bind_target_root`
    // refuses any optional link and the recursion propagates a deeper one up.
    assert_unsupported(
        "<script>const o = $state({el: null});</script><div bind:this={o?.el}></div>",
        "bind: directive this",
    );
    assert_unsupported(
        "<script>let o = $state({a: {b: null}});</script><div bind:this={o?.a.b}></div>",
        "bind: directive this",
    );
    assert_unsupported(
        "<script>let o = $state({a: {b: null}});</script><div bind:this={o.a?.b}></div>",
        "bind: directive this",
    );
    assert_unsupported(
        "<script>let o = $state({v: ''});</script><input bind:value={o?.v} />",
        "bind: directive value",
    );
    assert_unsupported(
        "<script>let o = $state({w: 0});</script><svelte:window bind:innerWidth={o?.w} />",
        "bind: directive innerWidth",
    );
    // The non-optional chain is untouched.
    compile(
        "<script>let o = $state({el: null});</script><div bind:this={o.el}></div>",
        &CompileOptions::default(),
    )
    .expect("a plain member chain must still compile");
}

#[test]
fn compile_template_scoped_const_bind_target_refuses() {
    // A `bind:` reaches the SAME validator as an assignment
    // (`BindDirective.js:181`), so the template-scoped consts obey the rule there
    // too — the oracle raises `constant_binding` on each of these (live-verified:
    // "Cannot bind to constant"). This was a TRACKED over-acceptance until the
    // template scopes were modeled; it is the bind half of
    // `compile_template_scoped_const_assignment_refuses`.
    assert_unsupported(
        "<script>let p = $state(0);</script>{#await p then v}<div bind:this={v}></div>{/await}",
        "a constant",
    );
    assert_unsupported(
        "<script>let c = $state(0);</script>{#if c}{@const o = {}}<div bind:this={o}></div>{/if}",
        "a constant",
    );
    assert_unsupported(
        "<script>let xs = [1];</script>{#each xs as x, i}<div bind:this={i}></div>{/each}",
        "a constant",
    );
}
