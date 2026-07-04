// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! `{#await}` `pending` fragment shape in the Svelte writer.
//!
//! In Svelte's parser (`1-parse/state/tag.js`), the **block form**
//! (`{#await x}…{/await}`) always creates a pending Fragment (`block.pending =
//! create_fragment()`), empty or not, while the inline `then`/`catch` shorthand
//! (`{#await x then v}` / `{#await x catch e}`) leaves `pending: null`. tsv
//! collapsed an empty block-form pending to `null`, losing that distinction.
//!
//! The fully-bare `{#await x}{/await}` is fixturable (it survives prettier) and is
//! pinned by `tests/fixtures/svelte/blocks/await/empty`. The block form *with* a
//! `{:then}`/`{:catch}` and an empty pending is **not** fixturable: both tsv and
//! prettier collapse `{#await x}{:then v}t` → `{#await x then v}t` (the shorthand),
//! so the block form never survives formatting — yet its *parse* must still carry
//! `pending: {Fragment, nodes: []}`. These root tests pin that wire distinction
//! (transcribed from the live modern Svelte parser).

use serde_json::Value;

/// Parse `src`, convert to the wire AST, and return the first `AwaitBlock`'s
/// `pending` field.
fn await_pending(src: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept the await block");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    find_await(&json).expect("an AwaitBlock")["pending"].clone()
}

fn find_await(node: &Value) -> Option<&Value> {
    match node {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("AwaitBlock") {
                return Some(node);
            }
            map.values().find_map(find_await)
        }
        Value::Array(items) => items.iter().find_map(find_await),
        _ => None,
    }
}

/// An empty Svelte `Fragment` is `{type: "Fragment", nodes: []}` — no positions.
fn assert_empty_fragment(pending: &Value) {
    assert_eq!(pending["type"], "Fragment", "pending should be a Fragment");
    assert_eq!(
        pending["nodes"].as_array().map(Vec::len),
        Some(0),
        "pending fragment should be empty"
    );
}

/// Block form, no `:then`/`:catch`, empty body → empty pending Fragment (also
/// covered by the `blocks/await/empty` fixture; kept here beside its siblings).
#[test]
fn bare_block_form_has_empty_pending_fragment() {
    assert_empty_fragment(&await_pending("{#await promise}{/await}"));
}

/// Block form with `{:then}` and an empty pending → empty Fragment, NOT null.
/// Not fixturable: the formatter collapses this to `{#await promise then v}t`.
#[test]
fn block_form_then_empty_pending_is_fragment() {
    assert_empty_fragment(&await_pending("{#await promise}{:then v}t{/await}"));
}

/// Block form with `{:catch}` and an empty pending → empty Fragment, NOT null.
#[test]
fn block_form_catch_empty_pending_is_fragment() {
    assert_empty_fragment(&await_pending("{#await promise}{:catch e}c{/await}"));
}

/// Inline `then` shorthand → `pending: null` (no pending phase).
#[test]
fn inline_then_shorthand_pending_is_null() {
    assert_eq!(
        await_pending("{#await promise then v}{v}{/await}"),
        Value::Null
    );
}

/// Inline `catch` shorthand → `pending: null`.
#[test]
fn inline_catch_shorthand_pending_is_null() {
    assert_eq!(
        await_pending("{#await promise catch e}c{/await}"),
        Value::Null
    );
}

/// Non-empty block-form pending is unaffected: the content Fragment is emitted.
#[test]
fn nonempty_block_form_pending_keeps_content() {
    let pending = await_pending("{#await promise}p{:then v}t{/await}");
    assert_eq!(pending["type"], "Fragment");
    assert_eq!(pending["nodes"][0]["type"], "Text");
    assert_eq!(pending["nodes"][0]["raw"], "p");
}
