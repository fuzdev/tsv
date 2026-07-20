//! Control-flow blocks: `{#if}`, `{#each}`, `{#await}`, `{#key}`, `{@const}`.

use super::support::*;

#[test]
fn compile_if_else_block() {
    // Branch anchors are single-quoted string pushes; the closer `<!--]-->`
    // is its own template push. A missing branch synthesizes nothing here.
    let js = compile_js("{#if a}<p>1</p>{:else}<p>2</p>{/if}");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tif (a) {\n\
             \t\t$$renderer.push('<!--[0-->');\n\
             \t\t$$renderer.push(`<p>1</p>`);\n\
             \t} else {\n\
             \t\t$$renderer.push('<!--[-1-->');\n\
             \t\t$$renderer.push(`<p>2</p>`);\n\
             \t}\n\
             \t$$renderer.push(`<!--]-->`);\n\
             }\n"
    );
}

#[test]
fn compile_if_synthesizes_missing_else() {
    // No `{:else}` → an anchor-only `else` branch with `<!--[-1-->`.
    let js = compile_js("{#if a}<p>1</p>{/if}");
    assert!(
        js.contains("} else {\n\t\t$$renderer.push('<!--[-1-->');\n\t}"),
        "missing else must be synthesized: {js}"
    );
}

#[test]
fn compile_else_if_chain_numbers_branches() {
    // Consequents number 0,1,…; the terminal else is -1; `else if` nests.
    let js = compile_js("{#if a}<p>1</p>{:else if b}<p>2</p>{:else}<p>3</p>{/if}");
    assert!(js.contains("if (a) {"), "{js}");
    assert!(js.contains("} else if (b) {"), "{js}");
    assert!(js.contains("$$renderer.push('<!--[0-->');"), "{js}");
    assert!(js.contains("$$renderer.push('<!--[1-->');"), "{js}");
    assert!(js.contains("$$renderer.push('<!--[-1-->');"), "{js}");
}

#[test]
fn compile_each_block() {
    let js = compile_js(
        "<script>let { items } = $props();</script>\n{#each items as item}<li>{item}</li>{/each}",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { items } = $$props;\n\
             \t$$renderer.push(`<!--[-->`);\n\
             \tconst each_array = $.ensure_array_like(items);\n\
             \tfor (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {\n\
             \t\tlet item = each_array[$$index];\n\
             \t\t$$renderer.push(`<li>${$.escape(item)}</li>`);\n\
             \t}\n\
             \t$$renderer.push(`<!--]-->`);\n\
             }\n"
    );
}

#[test]
fn compile_refuses_each_key_without_as() {
    // The oracle's `each_key_without_as` (`EachBlock.js:26-34`): a `(key)` with no
    // `as` clause, when keyed. The comma-index form `{#each x, i (key)}` is what
    // reaches here — a bare `{#each x (k)}` parses as a call `x(k)`, no key.
    // A member/expression key is always keyed.
    assert_unsupported(
        "<script>let items = [{ id: 1 }];</script>\n{#each items, i (items[i].id)}<p>{items[i].id}</p>{/each}",
        "{#each} with a key but no `as` clause",
    );
    // An identifier key naming something OTHER than the index is keyed.
    assert_unsupported(
        "<script>let items = [1];\nlet i = 0;</script>\n{#each items, j (i)}<p>x</p>{/each}",
        "{#each} with a key but no `as` clause",
    );
    // Discriminating controls, all COMPILE:
    // key === index → a plain indexed block, NOT keyed (the oracle's carve-out).
    let _ = compile_js("<script>let items = [1];</script>\n{#each items, i (i)}<p>{i}</p>{/each}");
    // a key WITH an `as` clause is the ordinary keyed each.
    let _ = compile_js(
        "<script>let items = [{ id: 1 }];</script>\n{#each items as item (item.id)}<p>{item.id}</p>{/each}",
    );
    // no key at all.
    let _ =
        compile_js("<script>let items = [1];</script>\n{#each items as item}<p>{item}</p>{/each}");
}

#[test]
fn compile_each_with_else_hoists_and_uses_authored_index() {
    // `{:else}` hoists `each_array` before an `if (…length !== 0)`; the
    // authored index name replaces `$$index` everywhere.
    let js = compile_js(
        "<script>let { items } = $props();</script>\n{#each items as item, i}<li>{i}</li>{:else}<p>none</p>{/each}",
    );
    assert!(
        js.contains(
            "const each_array = $.ensure_array_like(items);\n\tif (each_array.length !== 0) {"
        ),
        "each_array must hoist before the if: {js}"
    );
    assert!(js.contains("$$renderer.push('<!--[-->');"), "{js}");
    assert!(js.contains("$$renderer.push('<!--[!-->');"), "{js}");
    assert!(
        js.contains("for (let i = 0, $$length = each_array.length; i < $$length; i++) {"),
        "authored index must replace $$index: {js}"
    );
}

#[test]
fn compile_sibling_each_blocks_number_names() {
    // Sibling eachs get suffixed names in source order.
    let js = compile_js(
        "<script>let { a, b } = $props();</script>\n{#each a as x}<p>{x}</p>{/each}{#each b as y}<p>{y}</p>{/each}",
    );
    assert!(
        js.contains("const each_array = $.ensure_array_like(a);"),
        "{js}"
    );
    assert!(
        js.contains("const each_array_1 = $.ensure_array_like(b);"),
        "second each must be each_array_1: {js}"
    );
    assert!(js.contains("let x = each_array[$$index];"), "{js}");
    assert!(js.contains("let y = each_array_1[$$index_1];"), "{js}");
}

#[test]
fn compile_await_block_drops_catch() {
    // Always 4-arg `$.await`; the `{:catch}` branch is dropped entirely.
    let js = compile_js(
        "<script>let { p } = $props();</script>\n{#await p}<p>load</p>{:then v}<p>{v}</p>{:catch e}<p>err</p>{/await}",
    );
    assert!(js.contains("$.await("), "{js}");
    assert!(
        js.contains("(value) => {") || js.contains("(v) => {"),
        "then param: {js}"
    );
    assert!(js.contains("`<p>load</p>`"), "{js}");
    assert!(js.contains("$.escape(v)"), "{js}");
    assert!(!js.contains("err"), "catch content must be dropped: {js}");
    assert!(js.contains("$$renderer.push(`<!--]-->`);"), "{js}");
}

#[test]
fn compile_await_pending_only_has_empty_then() {
    // Pending-only await still emits 4 args with an empty `() => {}` then.
    let js = compile_js("<script>let { p } = $props();</script>\n{#await p}<p>load</p>{/await}");
    assert!(js.contains("() => {}"), "empty then arrow expected: {js}");
    assert!(js.contains("`<p>load</p>`"), "{js}");
}

#[test]
fn compile_key_block() {
    let js = compile_js("{#key a}<p>c</p>{/key}");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \t$$renderer.push(`<!---->`);\n\
             \t{\n\
             \t\t$$renderer.push(`<p>c</p>`);\n\
             \t}\n\
             \t$$renderer.push(`<!---->`);\n\
             }\n"
    );
}

#[test]
fn compile_const_tag_folds_static_read() {
    // A `{@const}` enters the evaluator: a statically-known init folds a read
    // into the template while the declaration still emits.
    let js = compile_js("{#if true}{@const x = 2}<p>{x}</p>{/if}");
    assert!(js.contains("const x = 2;"), "const decl must emit: {js}");
    assert!(
        js.contains("`<p>2</p>`"),
        "static const read must fold: {js}"
    );
    assert!(
        !js.contains("$.escape(x)"),
        "known read must not stay dynamic: {js}"
    );
}

#[test]
fn compile_const_tag_dynamic_read_stays_escaped() {
    // A `{@const}` over an unknown (each-local) value stays dynamic.
    let js = compile_js(
        "<script>let { items } = $props();</script>\n{#each items as item}{@const d = item}<p>{d}</p>{/each}",
    );
    assert!(js.contains("const d = item;"), "{js}");
    assert!(
        js.contains("$.escape(d)"),
        "dynamic const read must escape: {js}"
    );
}

#[test]
fn compile_marks_text_first_each_body_not_if_branch() {
    // The each body gets a `<!---->` text-first marker; the if branch does not.
    let each = compile_js(
        "<script>let { items } = $props();</script>\n{#each items as item}hi {item}{/each}",
    );
    assert!(each.contains("`<!---->hi ${$.escape(item)}`"), "{each}");
    let iff = compile_js("<script>let { a } = $props();</script>\n{#if a}hi {a}{/if}");
    assert!(
        iff.contains("$$renderer.push(`hi ${$.escape(a)}`);"),
        "if branch must NOT get a text-first marker: {iff}"
    );
}

#[test]
fn compile_rejects_nested_each() {
    assert_unsupported(
        "<script>let { m } = $props();</script>\n{#each m as row}{#each row as cell}<p>{cell}</p>{/each}{/each}",
        "nested {#each}",
    );
}

#[test]
fn compile_rejects_const_at_root() {
    assert_unsupported(
        "{@const x = 1}<p>text</p>",
        "{@const} at the component root",
    );
}

#[test]
fn compile_carries_comments_with_blocks() {
    // A script comment carries through as a leading comment of its surviving
    // statement, unaffected by a template block: the block emits template-region
    // spans only, so no comment window sweeps the script comment.
    let js = compile_js("<script>\n\t// note\n\tlet a = 1;\n</script>\n{#if a}<p>x</p>{/if}");
    assert!(
        js.contains("// note"),
        "the script comment must carry through: {js}"
    );
}
