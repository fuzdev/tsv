//! A regular element `{...spread}`: the fused `$.attributes(...)` call.

use super::support::*;

#[test]
fn compile_element_spread_object() {
    // A regular element `{...spread}` routes the WHOLE attribute set through one
    // fused `$.attributes({ … })` call, source order: plain attrs become object
    // properties, spreads become `...expr` elements.
    let js = compile_js(
        "<script>let props = $state({});</script>\n<div class=\"foo\" id=\"a\" {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ class: 'foo', id: 'a', ...props })"),
        "{js}"
    );
    // A single-expression event handler drops from the object; a bare boolean and
    // a `data-*` key (quoted, lowercased) survive.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state(1);</script>\n<div DataFoo={x} disabled onclick={x} {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ datafoo: x, disabled: true, ...props })"),
        "{js}"
    );
}

#[test]
fn compile_element_spread_flags_and_elision() {
    // `<input>` → the `ELEMENT_IS_INPUT` flag (4) with interior `void 0` padding.
    let js = compile_js("<script>let props = $state({});</script>\n<input {...props}/>");
    assert!(
        js.contains("$.attributes({ ...props }, void 0, void 0, void 0, 4)"),
        "{js}"
    );
    // A custom element (hyphenated tag) → `ELEMENT_PRESERVE_ATTRIBUTE_CASE` (2).
    let js = compile_js("<script>let props = $state({});</script>\n<my-elem {...props}></my-elem>");
    assert!(
        js.contains("$.attributes({ ...props }, void 0, void 0, void 0, 2)"),
        "{js}"
    );
}

#[test]
fn compile_element_spread_scope_hash_rides_second_arg() {
    // In spread mode the scope hash is NOT concatenated into the class value — it
    // rides the `css_hash` (2nd) argument.
    let out = compile_checked(
        "<script>let props = $state({});</script>\n<div class=\"foo\" {...props}></div><style>.foo{color:red}</style>",
    );
    assert!(
        out.js
            .contains("$.attributes({ class: 'foo', ...props }, 'svelte-tsvhash')"),
        "{}",
        out.js
    );
}

#[test]
fn compile_element_spread_prop_root_forces_context_wrapper() {
    // A member access rooted at a prop inside a `{...spread}` must fire the
    // `$$renderer.component` wrapper (the reference feeds `needs_context`).
    let out = compile_checked("<script>let obj = $props();</script>\n<div {...obj.foo}></div>");
    assert!(
        out.js.contains("$$renderer.component(($$renderer) =>"),
        "prop-rooted spread must wrap: {}",
        out.js
    );
}

#[test]
fn compile_element_spread_with_class_and_style_directives() {
    // A `class:`/`style:` directive co-present with a `{...spread}` folds into the
    // `classes` (3rd) / `styles` (4th) `$.attributes` arguments — an identifier-key
    // object with shorthand collapse for `classes`, a FLAT object (no `|important`
    // partition) for `styles`.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state(1); let v = $state('');</script>\n<div class:a={x} style:color={v} {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, void 0, { a: x }, { color: v })"),
        "{js}"
    );
    // A shorthand `class:active` collapses to `{ active }`.
    let js = compile_js(
        "<script>let props = $state({}); let active = $state(true);</script>\n<div class:active {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, void 0, { active })"),
        "{js}"
    );
    // `|important` is validated but does NOT partition in spread mode.
    let js = compile_js(
        "<script>let props = $state({}); let v = $state('');</script>\n<div style:c|important={v} {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, void 0, void 0, { c: v })"),
        "{js}"
    );
}

#[test]
fn compile_element_spread_bind_folds_into_object() {
    // A `bind:value` folds into the object at the bind's source slot (before the
    // spread); `<input>` still sets the flags argument.
    let js = compile_js(
        "<script>let props = $state({}); let w = $state('');</script>\n<input bind:value={w} {...props}/>",
    );
    assert!(
        js.contains("$.attributes({ value: w, ...props }, void 0, void 0, void 0, 4)"),
        "{js}"
    );
    // `bind:group` synthesizes a `checked` entry; the companion `value` still emits
    // as its own object property.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state('a');</script>\n<input type=\"radio\" bind:group={x} value=\"a\" {...props}/>",
    );
    assert!(
        js.contains(
            "$.attributes({ type: 'radio', checked: x === 'a', value: 'a', ...props }, void 0, void 0, void 0, 4)"
        ),
        "{js}"
    );
    // All together: bind entry in the object, class/style args, input flags.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state(1); let v = $state(''); let w = $state('');</script>\n<input class:a={x} style:color={v} bind:value={w} {...props}/>",
    );
    assert!(
        js.contains("$.attributes({ value: w, ...props }, void 0, { a: x }, { color: v }, 4)"),
        "{js}"
    );
}

#[test]
fn compile_element_spread_directive_scoping_and_drops() {
    // A `class:` directive NAME matching a scoped selector scopes the element — the
    // hash rides the `css_hash` (2nd) argument, the classes object the 3rd.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state(1);</script>\n<div class:foo={x} {...props}></div>\n<style>.foo { color: red }</style>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, 'svelte-tsvhash', { foo: x })"),
        "{js}"
    );
    // The drop family (`use:`/`transition:`) contributes nothing — a bare
    // `$.attributes({ ...props })`.
    let js =
        compile_js("<script>let props = $state({});</script>\n<div use:action {...props}></div>");
    assert!(js.contains("$.attributes({ ...props })"), "{js}");
    let js = compile_js(
        "<script>let props = $state({});</script>\n<div transition:fade {...props}></div>",
    );
    assert!(js.contains("$.attributes({ ...props })"), "{js}");
}

#[test]
fn compile_element_spread_refuses_invalid_directives() {
    // A `bind:value` on a non-`<input>` element is `bind_invalid_target` (an oracle
    // error) — the slice-3 gate still applies with a spread.
    assert_unsupported(
        "<script>let props = $state({}); let v = $state('');</script>\n<div bind:value={v} {...props}></div>",
        "bind: directive value",
    );
    // A `style:` directive with an invalid modifier still refuses.
    assert_unsupported(
        "<script>let props = $state({}); let v = $state('');</script>\n<div style:color|foo={v} {...props}></div>",
        "style: directive with an invalid modifier",
    );
    // A deferred (content-editable) bind still refuses.
    assert_unsupported(
        "<script>let props = $state({}); let h = $state('');</script>\n<div contenteditable=\"true\" bind:innerHTML={h} {...props}></div>",
        "bind: directive innerHTML",
    );
    // A legacy `on:` directive and `let:` alongside a spread stay refused (the
    // oracle drops them, but tsv declines to reproduce that).
    assert_unsupported(
        "<script>let props = $state({});</script>\n<div on:click={() => {}} {...props}></div>",
        "legacy on: directive (runes-only fence)",
    );
    assert_unsupported(
        "<script>let props = $state({});</script>\n<div let:x {...props}></div>",
        "legacy let: directive (runes-only fence)",
    );
}

#[test]
fn compile_element_spread_refuses_omit_in_ssr_binds() {
    // An `omit_in_ssr` bind (media/dimension/window binding) co-present with a
    // `{...spread}` refuses on the spread path too — consistent with the inline
    // path, and the SAFE side (the oracle rejects these shapes; tsv declines rather
    // than silently drop them). Well-formed `omit_in_ssr`+spread parity is deferred.
    let prefix =
        "<script>let props = $state({}); let w = $state(''); let x = $state(1);</script>\n";
    // `bind:files` needs `type=\"file\"` (an oracle `bind_invalid_target`).
    assert_unsupported(
        &format!("{prefix}<input bind:files={{w}} {{...props}}/>"),
        "bind: directive files",
    );
    // A dimension binding on a non-matching element (oracle `bind_invalid_target`).
    assert_unsupported(
        &format!("{prefix}<div bind:clientWidth={{x}} {{...props}}></div>"),
        "bind: directive clientWidth",
    );
    // A window binding on a non-window element (oracle `bind_invalid_target`).
    assert_unsupported(
        &format!("{prefix}<div bind:scrollX={{w}} {{...props}}></div>"),
        "bind: directive scrollX",
    );
    // A non-lvalue target on an `omit_in_ssr` bind (oracle `bind_invalid_expression`).
    assert_unsupported(
        &format!("{prefix}<div bind:clientWidth={{f()}} {{...props}}></div>"),
        "bind: directive clientWidth",
    );
}
