// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The `no-locations` wire variant invariant.
//!
//! `convert_ast_json_bytes_no_locations` must emit exactly the default wire with
//! every line/column object removed — nothing else. Concretely: the default
//! output with all `loc` keys stripped (and, for Svelte, `name_loc` — which the
//! variant also drops) must equal the no-locations output byte-for-byte (compared
//! as parsed `Value`s). This proves the variant drops *only* line/column data and
//! leaves `type`/`start`/`end`/payload untouched — so a stray field change, a
//! dropped comma, or a reordered key in the gated writer is caught here.
//!
//! No new oracle files: the default `expected.json` (already gated against the
//! canonical parsers) is the source of truth; `strip_locations` derives the
//! expected no-locations shape from it.

use serde_json::Value;

/// Recursively remove every `loc` and `name_loc` key — the exact set the
/// `no-locations` variant omits. `character` lives inside `loc`, so it goes too.
fn strip_locations(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.remove("loc");
            map.remove("name_loc");
            for child in map.values_mut() {
                strip_locations(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_locations(item);
            }
        }
        _ => {}
    }
}

fn assert_ts(src: &str) {
    let arena = bumpalo::Bump::new();
    let ast = tsv_ts::parse(src, &arena).expect("TS source should parse");
    let mut full = tsv_ts::convert_ast_json(&ast, src);
    strip_locations(&mut full);
    let no_loc: Value =
        serde_json::from_slice(&tsv_ts::convert_ast_json_bytes_no_locations(&ast, src))
            .expect("no-locations output is valid JSON");
    assert_eq!(
        full, no_loc,
        "TS no-locations != strip_loc(full) for: {src:?}"
    );
}

fn assert_svelte(src: &str) {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("Svelte source should parse");
    let mut full = tsv_svelte::convert_ast_json(&ast, src);
    strip_locations(&mut full);
    let no_loc: Value =
        serde_json::from_slice(&tsv_svelte::convert_ast_json_bytes_no_locations(&ast, src))
            .expect("no-locations output is valid JSON");
    assert_eq!(
        full, no_loc,
        "Svelte no-locations != strip_loc(full) for: {src:?}"
    );
}

#[test]
fn ts_empty_program() {
    assert_ts("");
}

#[test]
fn ts_ascii_statements() {
    assert_ts("const a = 1;\nfunction f(x) { return x + 1; }\nlet y = f(a);");
}

#[test]
fn ts_multibyte_identifier() {
    // A multibyte char before later nodes exercises the byte→UTF-16 offset path
    // that `start`/`end` still depend on (only line/column is dropped).
    assert_ts("const café = 1;\nconst π = café + 2;\nconst 𝕏 = π;");
}

#[test]
fn ts_crlf_and_line_separators() {
    // CRLF (one terminator) and U+2028 exercise the ECMAScript line rules that
    // the dropped `loc` would encode — the no-loc output must still match.
    assert_ts("const a = 1;\r\nconst b = 2;\u{2028}const c = 3;");
}

#[test]
fn ts_types_and_comments() {
    assert_ts(
        "// leading\ninterface I { a: number; b?: string }\nconst x: I = /* inline */ { a: 1, b: 'y' };",
    );
}

#[test]
fn svelte_elements_attributes_directives() {
    // Elements/attributes/directives carry `name_loc`; the variant drops it.
    assert_svelte(
        "<div class=\"a\" on:click={handle} bind:value={v} class:active={on}>\n\t<span>{text}</span>\n</div>",
    );
}

#[test]
fn svelte_script_and_expressions() {
    // `<script>` acorn nodes carry `loc`; `{expr}` islands too.
    assert_svelte(
        "<script lang=\"ts\">\n\tlet count: number = 0;\n\tconst café = count;\n</script>\n\n<button on:click={() => count++}>{count}</button>",
    );
}

#[test]
fn svelte_blocks_and_comments() {
    assert_svelte(
        "<!-- a comment -->\n{#if ready}\n\t{#each items as item, i}\n\t\t<li>{item.name}</li>\n\t{/each}\n{/if}",
    );
}

#[test]
fn svelte_const_tag_and_snippet() {
    assert_svelte(
        "{#snippet row(name)}\n\t{@const upper = name.toUpperCase()}\n\t<td>{upper}</td>\n{/snippet}",
    );
}

/// The **comment-attach** path, which the cases above never reach: a
/// `leadingComments` / `trailingComments` list is emitted only for a
/// comment-bearing Svelte island, and its per-node assignment is driven by the
/// writer's own node opens (`attach_open`) rather than by anything `loc`-shaped.
///
/// ⭐ That independence is what these cases gate, and nothing else does. The
/// attach walks the emit, so an `attach_open` that ever became conditional on
/// `emit_loc` would silently stop opening nodes in *this* variant only —
/// comments would re-attach to whatever node opened next, and every other gate
/// would stay green, because they all run with `loc` on. Mutation-tested:
/// gating `attach_open` on `ctx.emit_loc` passes the whole workspace suite
/// without these.
#[test]
fn svelte_script_attached_comments() {
    // Leading + trailing on a statement, and a comment inside a nested block —
    // the `is_last_in_body` widening the attach reads off the emit.
    assert_svelte(
        "<script lang=\"ts\">\n\t// lead\n\tconst a: number = 1; /* t */\n\tfunction f() {\n\t\tg(); // inner\n\t}\n</script>",
    );
}

#[test]
fn svelte_preceding_html_comment_attached() {
    // The positionless `{type: "Line", value}` Svelte prepends to the `<script>`
    // `Program`'s `leadingComments` — the one attached comment with no span.
    assert_svelte("<!-- @component doc -->\n<script>let x = 1;</script>");
}

#[test]
fn svelte_template_island_attached_comments() {
    // A template expression island: leading and trailing on the same node.
    assert_svelte("<div>{/* l */ value /* t */}</div>");
}

#[test]
fn svelte_pattern_island_attached_comments() {
    // A block binding pattern and a `{@const}` id — the two islands whose window
    // runs to the end of the binding rather than of the root node.
    assert_svelte("{#each items as { a = /* c */ 1 }}<li>x</li>{/each}");
    assert_svelte("{@const { a = /* c */ 1 } = o}");
}

#[test]
fn svelte_expression_list_island_attached_comments() {
    // A `{#snippet}` parameter list — one shared queue across several roots.
    assert_svelte("{#snippet s(a, /* c */ b)}x{/snippet}");
}

/// A **multi-line block** comment, which every case above deliberately isn't: its `value` is
/// the one comment field that is not a verbatim source slice. Svelte's `onComment` dedents it
/// by the line of the source *its own parse* was handed, and Svelte manufactures that source
/// at four of its readers (`tsv_lang::AcornPrefix`) — so the writer must resolve the
/// preparation per island on **both** variants.
///
/// ⭐ That is what this case gates, and nothing else does. The preparation ledger
/// (`Root::acorn_regions`) sits beside the `loc` seeds, which the `no-locations` path builds
/// none of; parked with them — its pre-refactor home — the dedent silently reverts to the
/// document on this variant alone, and every other gate stays green because they all run with
/// `loc` on. The newline before the `:` is load-bearing twice over: it is what makes the
/// annotation's seed non-identity (so the seed route is live and the two could be conflated),
/// and `read_type_annotation`'s `_ as ` swallows it, so the line acorn measured is the
/// binding's.
#[test]
fn svelte_manufactured_multiline_comment_dedent() {
    assert_svelte(
        "<script lang=\"ts\">\n\tlet xs = [1];\n</script>\n\
         {#each xs as x\n\t: /* c1\n\t c2 */ number}{x}{/each}",
    );
}
