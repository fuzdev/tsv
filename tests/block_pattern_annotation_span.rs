// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used, clippy::panic)]

//! A Svelte block binding's `TSTypeAnnotation` starts where the **binding** ends, not at the
//! colon — and the two are the same position only when nothing separates them.
//!
//! Svelte builds this node itself rather than taking acorn's
//! (`svelte/packages/svelte/src/compiler/phases/1-parse/read/context.js`):
//!
//! ```js
//! const start = parser.index;   // <- before the skip
//! parser.allow_whitespace();
//! if (!parser.eat(':')) { … }
//! return { type: 'TSTypeAnnotation', start, end: parser.index, … };
//! ```
//!
//! So any whitespace the author put between the binding and its `:` belongs to the
//! annotation's span. This is a **Svelte-only** convention: acorn-typescript anchors its own
//! `TSTypeAnnotation` at the colon (`function f(a : number)` starts the node at the `:`), and
//! tsv matches it there — which is why the re-anchor lives at
//! `tsv_ts::attach_pattern_type_annotation`, the attach point only the two Svelte block
//! readers call, rather than in the shared type parser.
//!
//! **Why a test rather than a fixture.** Both formatters collapse `e : T` to `e: T`, so a
//! document carrying the gap is not the fixed point F1 requires. The adjacent spelling — the
//! one every fixture and essentially all real code uses — is the null control below, and it
//! is the reason this went unseen: with the colon glued to the binding the two conventions
//! agree exactly.

use serde_json::Value;

/// The `[start, end)` of the one `TSTypeAnnotation` in the wire.
fn annotation_span(src: &str) -> (u64, u64) {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept the component");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let mut found = Vec::new();
    collect(&json, &mut found);
    match found.as_slice() {
        [one] => *one,
        other => panic!(
            "expected exactly one TSTypeAnnotation, found {}",
            other.len()
        ),
    }
}

fn collect(node: &Value, out: &mut Vec<(u64, u64)>) {
    match node {
        Value::Object(fields) => {
            if fields.get("type").and_then(Value::as_str) == Some("TSTypeAnnotation") {
                let n = |k: &str| fields.get(k).and_then(Value::as_u64).expect("span field");
                out.push((n("start"), n("end")));
            }
            for value in fields.values() {
                collect(value, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect(i, out)),
        _ => {}
    }
}

/// Byte offset of `needle` in `src`, which must occur exactly once.
fn only_offset(src: &str, needle: &str) -> u64 {
    let at = src.find(needle).expect("needle should occur");
    assert!(
        !src[at + 1..].contains(needle),
        "needle {needle:?} should occur once"
    );
    at as u64
}

const HEAD: &str =
    "<script lang=\"ts\">\n\tlet xs = [1];\n\tlet p = Promise.resolve(1);\n</script>\n\n";

/// The three block readers that admit a typed binding, each with a space before the colon.
/// The annotation starts at the space — the binding's end — not at the `:`.
#[test]
fn a_space_before_the_colon_belongs_to_the_annotation() {
    for (label, src, binding) in [
        (
            "each identifier",
            format!("{HEAD}{{#each xs as e : number}}\n\t{{e}}\n{{/each}}\n"),
            "e ",
        ),
        (
            "each destructure",
            // A bare type reference, not an object type: a `{ a: number }` literal carries
            // its own member `TSTypeAnnotation` and this asserts about exactly one.
            format!("{HEAD}{{#each xs as {{ a }} : Rec}}\n\t{{a}}\n{{/each}}\n"),
            "} ",
        ),
        (
            "then",
            format!("{HEAD}{{#await p}}\n\t...\n{{:then v : number}}\n\t{{v}}\n{{/await}}\n"),
            "v ",
        ),
    ] {
        // The binding's end: the offset just past its last character.
        let expected_start = only_offset(&src, binding) + binding.len() as u64 - 1;
        assert_eq!(annotation_span(&src).0, expected_start, "{label}");
    }
}

/// The null control: with the colon glued to the binding, "binding end" and "colon" are the
/// same offset, so the wire is identical under either convention. Every fixture in the tree
/// is this shape — which is exactly why the divergence above stayed invisible.
#[test]
fn an_adjacent_colon_leaves_the_two_conventions_agreeing() {
    let src = format!("{HEAD}{{#each xs as e: number}}\n\t{{e}}\n{{/each}}\n");
    assert_eq!(annotation_span(&src).0, only_offset(&src, "e:") + 1);
}
