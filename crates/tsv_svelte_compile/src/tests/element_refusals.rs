//! Per-element refusals: duplicate `$props()`, the select family, `<svelte:element>`.

use super::support::*;
use crate::*;

#[test]
fn compile_duplicate_props_rune_refuses() {
    // The oracle's `props_duplicate` rejects a second `$props()`. Corpus-invisible
    // (no real component writes it), so this test is the only guard.
    assert_unsupported(
        "<script>let {a} = $props(); let {b} = $props();</script>{a}{b}",
        "$props() used more than once",
    );
    assert_unsupported(
        "<script>let {a} = $props(); let {b} = $props(); let {c} = $props();</script>",
        "$props() used more than once",
    );
    // Both declarators in ONE statement is the same duplicate.
    assert_unsupported(
        "<script>let {a} = $props(), {b} = $props();</script>{a}{b}",
        "$props() used more than once",
    );
    // A non-destructured second `$props()` duplicates just the same.
    assert_unsupported(
        "<script>let {a} = $props(); let rest = $props();</script>{a}",
        "$props() used more than once",
    );
    // A single `$props()` still compiles — the guard must not fire on one.
    compile(
        "<script>let {a} = $props();</script>{a}",
        &CompileOptions::default(),
    )
    .expect("a single $props() must still compile");
    // `$props()` and `$props.id()` are tracked separately (the oracle keeps two
    // flags), so one of each is NOT a duplicate.
    compile(
        "<script>let {a} = $props(); let i = $props.id();</script>{a}{i}",
        &CompileOptions::default(),
    )
    .expect("$props() alongside $props.id() must still compile");
}

#[test]
fn compile_use_directive_on_load_error_element_refuses() {
    // `use:` on a load-error element makes the oracle add onload/onerror capture
    // attributes (its `events_to_capture` set) — not implemented, so refuse.
    // Only `use:` (and a spread) triggers this; the other drop-family kinds drop.
    assert_unsupported("<img use:action />", "load-error element");
    assert_unsupported("<iframe use:action></iframe>", "load-error element");
    // `transition:`/`{@attach}` on the same element are a plain drop.
    let out = compile("<img transition:fade />", &CompileOptions::default()).unwrap();
    assert!(
        out.js.contains("`<img/>`"),
        "transition on img must plain-drop: {}",
        out.js
    );
    let out = compile("<img {@attach a} />", &CompileOptions::default()).unwrap();
    assert!(
        out.js.contains("`<img/>`"),
        "attach on img must plain-drop: {}",
        out.js
    );
}

#[test]
fn compile_await_in_dropped_directive_expression_refuses() {
    // The oracle rejects `await` inside a directive expression
    // (`illegal_await_expression` / the async gate); tsv's dropped-expression
    // guard refuses the top-level await, the correct analog.
    assert_unsupported("<div use:action={await f()}></div>", "top-level await");
    assert_unsupported("<div {@attach await mk()}></div>", "top-level await");
}

#[test]
fn compile_rune_in_dropped_directive_expression_refuses() {
    // A dropped directive expression is still validated: a misplaced rune is an
    // oracle analysis-phase error (`state_invalid_placement`), so tsv refuses.
    assert_unsupported("<div use:action={$state(1)}></div>", "rune $state");
    assert_unsupported("<div {@attach $derived(1)}></div>", "rune $derived");
}

#[test]
fn compile_select_family_spread_and_bind_refuse() {
    // The `<select>` trap: an empty `<select {...props}>` / `<select bind:value>`
    // routes through `$$renderer.select(...)` in the oracle, NOT `$.attributes`.
    // Spread and `bind:` are refused in this slice, so both refuse today — pin it
    // so the later spread/bind slices can't silently mis-route the select family.
    // See docs/checklist_svelte_compiler.md §select-family.
    assert_unsupported("<select {...props}></select>", "{...spread} on <select>");
    assert_unsupported(
        "<script>let v = $state('');</script><select bind:value={v}></select>",
        "bind: directive value",
    );
}

#[test]
fn compile_svelte_element_const_tag_direct_child_refuses() {
    // The oracle rejects a `{@const}` as a direct `<svelte:element>` child
    // (`const_tag_invalid_placement`; a `<svelte:element>` is not among its valid
    // `{@const}` parents). Without a guard tsv would over-accept: the children
    // closure pushes a block-scope overlay (load-bearing for snippet hoisting) that
    // `emit_const_tag` reads as "inside a block". Pin the refusal.
    assert_unsupported(
        "<svelte:element this={tag}>{@const y = 1}{y}</svelte:element>",
        "{@const} outside a block scope",
    );
    // A `{#snippet}` direct child stays valid (proves the guard didn't drop the
    // overlay the hoist analysis needs).
    compile(
        "<svelte:element this={tag}>{#snippet s()}x{/snippet}{@render s()}</svelte:element>",
        &CompileOptions::default(),
    )
    .expect("a {#snippet} child of <svelte:element> still compiles");
}

#[test]
fn compile_svelte_element_specific_refusals() {
    // A `bind:` other than `bind:this` refuses — the dynamic tag has no static
    // `<input>` identity, so the oracle rejects `bind:value`/etc.
    // (`bind_invalid_target`).
    assert_unsupported(
        "<script>let x = $state(0);</script><svelte:element this={tag} bind:value={x} />",
        "bind: directive value",
    );
    // Legacy `on:`/`let:` refuse (the runes-only fence).
    assert_unsupported(
        "<svelte:element this={tag} on:click={h} />",
        "legacy on: directive (runes-only fence)",
    );
    // A scoping `<style>` scopes the element: a type selector matches a
    // `<svelte:element>` unconditionally, so it synthesizes the hash class and the
    // selector is used (not pruned → no `CssSelectorNoMatch`).
    let out = compile(
        "<svelte:element this={tag} /><style>div { color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("scoped <svelte:element> compiles");
    assert!(
        out.js.contains(r#" class="svelte-tsvhash""#),
        "expected synthesized hash class, got: {}",
        out.js
    );
    assert!(
        out.css
            .as_deref()
            .is_some_and(|css| css.contains("div.svelte-tsvhash")),
        "expected scoped selector, got: {:?}",
        out.css
    );
    // A `bind:this` omits and the element compiles.
    compile(
        "<script>let el;</script><svelte:element this={tag} bind:this={el} />",
        &CompileOptions::default(),
    )
    .expect("bind:this on <svelte:element> compiles");
}
