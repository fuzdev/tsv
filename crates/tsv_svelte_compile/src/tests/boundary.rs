//! `<svelte:boundary>`: attribute validation, anchors, snippet forms.

use super::support::*;
use crate::*;

#[test]
fn compile_boundary_invalid_attributes_refuse() {
    // The oracle's phase-2 `SvelteBoundary` visitor accepts a CLOSED list of three
    // names, each carrying exactly one `{expression}`. tsv's parser accepts all of
    // the shapes below, so without the guard each would be an over-acceptance of
    // input the oracle rejects — the refusal contract's hard bar.
    // `svelte_boundary_invalid_attribute`: an unknown plain attribute, a spread,
    // and every directive kind (a modern event attribute included).
    for source in [
        "<svelte:boundary foo={1}><p>a</p></svelte:boundary>",
        "<svelte:boundary {...x}><p>a</p></svelte:boundary>",
        "<svelte:boundary onclick={f}><p>a</p></svelte:boundary>",
        "<svelte:boundary on:click={f}><p>a</p></svelte:boundary>",
        "<svelte:boundary class:x={y}><p>a</p></svelte:boundary>",
        "<svelte:boundary bind:this={el}><p>a</p></svelte:boundary>",
        "<svelte:boundary use:action><p>a</p></svelte:boundary>",
    ] {
        assert_unsupported(source, "invalid attribute on <svelte:boundary>");
    }
    // `svelte_boundary_invalid_attribute_value`: a valid NAME whose value is not
    // exactly one expression tag — a DIFFERENT refusal, pinned separately so the
    // two cannot collapse into each other.
    for source in [
        "<svelte:boundary onerror><p>a</p></svelte:boundary>",
        "<svelte:boundary onerror=\"x\"><p>a</p></svelte:boundary>",
        "<svelte:boundary onerror=\"a{b}c\"><p>a</p></svelte:boundary>",
    ] {
        assert_unsupported(
            source,
            "non-expression value for <svelte:boundary> attribute onerror",
        );
    }
}

#[test]
fn compile_boundary_snippet_attribute_forms_refuse() {
    // The `failed={expr}` / `pending={expr}` ATTRIBUTE forms compile in the oracle
    // but are a deliberate v1 gap (asymmetric precedence against a same-named
    // snippet, plus the statically-nullish `pending` if/else fork).
    assert_unsupported(
        "<script>const f = () => {};</script><svelte:boundary failed={f}><p>a</p></svelte:boundary>",
        "attribute form",
    );
    assert_unsupported(
        "<script>const g = () => {};</script><svelte:boundary pending={g}><p>a</p></svelte:boundary>",
        "attribute form",
    );
}

#[test]
fn compile_boundary_onerror_is_dropped_but_guarded() {
    // A valid `onerror={handler}` never reaches SSR output, but the oracle still
    // analyzes it — so it is guard-walked like an event-handler attribute.
    let out = compile(
        "<svelte:boundary onerror={(e) => e}><p>a</p></svelte:boundary>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(!out.js.contains("onerror"), "onerror must drop: {}", out.js);
    assert_unsupported(
        "<svelte:boundary onerror={() => $state(1)}><p>a</p></svelte:boundary>",
        "rune $state",
    );
}

#[test]
fn compile_boundary_anchors_are_isolated_pushes() {
    // Unlike `{#key}`'s `<!---->` marker, a boundary's anchors do NOT merge into an
    // adjacent sibling's template — the oracle's `build_template` starts a fresh
    // push for each. A merge would be invisible to the fixtures if no fixture put a
    // sibling beside a boundary, so pin it here too.
    let out = compile(
        "<b>a</b><svelte:boundary><p>hi</p></svelte:boundary><i>z</i>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$$renderer.push(`<b>a</b>`)")
            && out.js.contains("$$renderer.push(`<!--[-->`)"),
        "anchors must not merge with siblings: {}",
        out.js
    );
}
