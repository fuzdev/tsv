//! `$$slots`, `<svelte:head>`, `<title>`, and the client-generation refusal.

use super::support::*;
use crate::*;

#[test]
fn compile_slots_reference_injects_sanitize() {
    // A `$$slots` reference injects the binding and takes `$$props`.
    let out = compile_checked("<p>{$$slots}</p>");
    assert!(
        out.js.contains("const $$slots = $.sanitize_slots($$props)")
            && out.js.contains("function Input($$renderer, $$props)"),
        "sanitize_slots injection missing: {}",
        out.js
    );
}

#[test]
fn compile_carries_comments_with_slots() {
    // The injected `const $$slots = $.sanitize_slots($$props)` is zero-width at the
    // body start, so it opens no window over the script: a comment inside a function
    // body prints once, not also swept ahead of the injected statement.
    let js = compile_js(
        "<script>\n\t// note\n\tlet x = 1;\n\tfunction f() {\n\t\t// inner\n\t\treturn $$slots.default;\n\t}\n</script>\n<p>{x}{f()}</p>",
    );
    assert_eq!(js.matches("// note").count(), 1, "{js}");
    assert_eq!(js.matches("// inner").count(), 1, "{js}");
}

#[test]
fn compile_slots_with_props_rest_renames_destructured_slots() {
    // The injected sanitize_slots const owns `$$slots`, so the rest-props
    // injection deconflicts by renaming: `$$slots: $$slots_` (a shorthand
    // `$$slots` would be a duplicate lexical declaration — invalid JS).
    let out = compile_checked("<script>let {...r} = $props();</script><p>{$$slots}{r}</p>");
    assert!(
        out.js.contains("const $$slots = $.sanitize_slots($$props)")
            && out.js.contains("{ $$slots: $$slots_, $$events, ...r }"),
        "rest-props $$slots rename wrong: {}",
        out.js
    );
    // Non-destructured `let props = $props()` deconflicts the same way.
    let out = compile_checked("<script>let props = $props();</script><p>{$$slots}{props}</p>");
    assert!(
        out.js.contains("{ $$slots: $$slots_, $$events, ...props }"),
        "non-destructured $$slots rename wrong: {}",
        out.js
    );
    // Without a `$$slots` reference the injection stays shorthand.
    let out = compile_checked("<script>let {...r} = $props();</script><p>{r}</p>");
    assert!(
        out.js.contains("{ $$slots, $$events, ...r }"),
        "shorthand injection regressed: {}",
        out.js
    );
}

#[test]
fn compile_svelte_head_emits_head_call() {
    // `<svelte:head>` → `$.head('<hash>', $$renderer, closure)`. The hash is
    // the ported `hash("input.svelte")`.
    let out = compile_checked("<svelte:head><meta charset=\"utf-8\" /></svelte:head>");
    assert!(
        out.js
            .contains("$.head('4hbqx4', $$renderer, ($$renderer) =>"),
        "head call wrong: {}",
        out.js
    );
}

#[test]
fn compile_svelte_head_title_emits_title_call() {
    // `<title>` inside `<svelte:head>` → `$$renderer.title(($$renderer) => …)`,
    // hoisted before any sibling head content.
    let out = compile_checked("<svelte:head><title>Hi</title></svelte:head>");
    assert!(
        out.js.contains("$$renderer.title(($$renderer) =>")
            && out.js.contains("$$renderer.push(`<title>Hi</title>`)"),
        "title call wrong: {}",
        out.js
    );
}

#[test]
fn compile_rejects_title_attribute() {
    // Any attribute on `<title>` is `title_illegal_attribute` in the oracle; tsv's
    // parser accepts it, so the compiler refuses.
    assert_unsupported(
        "<svelte:head><title foo=\"x\">Hi</title></svelte:head>",
        "attribute on <title>",
    );
}

#[test]
fn compile_rejects_title_invalid_content() {
    // A `<title>` child that is neither text nor `{expression}` is
    // `title_invalid_content` in the oracle; tsv's parser accepts it, so refuse.
    assert_unsupported(
        "<svelte:head><title>{#if x}a{/if}</title></svelte:head>",
        "invalid <title> content",
    );
}

#[test]
fn compile_rejects_client_generation() {
    let options = CompileOptions {
        generate: Generate::Client,
        dev: false,
    };
    let err = compile("<p>text</p>", &options).unwrap_err();
    assert!(
        matches!(err, CompileError::Unsupported(_)),
        "expected Unsupported, got {err:?}"
    );
}
