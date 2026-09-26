// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A typed Svelte block binding (`{#each xs as { a }: T}`, `{:then v: T}`, `{:catch e: E}`,
//! `{@const { a }: T = x}`) is **two** acorn parses, and each attaches its own comments.
//!
//! Svelte's `read_pattern` parses the pattern as a synthetic `(pattern = 1)` expression, and
//! `read_type_annotation` then parses the `: T` separately through its `_ as ` trick
//! (`svelte/packages/svelte/src/compiler/phases/1-parse/read/context.js`). `add_comments`
//! runs once per parse, over that parse's own comments, so a comment written inside the
//! pattern can only attach to a node of the pattern — never to the annotation — and one
//! written inside the annotation can only attach inside the annotation. A comment neither
//! parse's walk claims attaches nowhere: the pattern parse leaves it leading the discarded
//! `= 1` literal, and the annotation parse leaves it to the discarded `_ as` wrapper, whose
//! trailing fallback needs the comment to start past it.
//!
//! The shapes that can tell one window from two are the comments a walk leaves unclaimed:
//! an own-line comment before the pattern's closing bracket (acorn's single trailing claim
//! needs a `/^[,) \t]*$/` gap, and a newline is not in it), and an own-line comment before
//! the closing brace of an object type in the annotation. One window over both parses lets
//! the first lead the `TSTypeAnnotation` that opens after it, and lets the second fall back
//! to trail the pattern root, whose span stops at the bare pattern.
//!
//! **Why a test rather than a fixture.** Prettier-plugin-svelte drops every comment in these
//! four block patterns, so a fixture holding one is a `_prettier_divergence` whose layout is
//! a claim of its own; the claim here is wire-only. `{@const}` has no such problem — its
//! layout is prettier's and a fixed point — so its cells live in the parser fixture
//! [const_destructure_annotation_comment](../tests/fixtures/svelte/tags/const/const_destructure_annotation_comment/),
//! and are repeated below only so the four hosts read in one place. Every expectation is
//! transcribed from the live modern parser (`cargo run -p tsv_debug canonical_parse`), so it
//! would go stale silently if Svelte changed. Attachments are compared as a SET: under
//! `lang="ts"` canonical lists every `{#each}` pattern comment twice and attaches both
//! copies (the cataloged
//! [ts_head_comment_duplication](../tests/fixtures/svelte/blocks/each/ts_head_comment_duplication_svelte_prettier_divergence/)
//! divergence), which is a question of how many, not where.

use serde_json::Value;

const HEAD: &str = "<script lang=\"ts\"></script>\n\n";

/// One attached comment: `(owner node type, list key, comment value)`.
type Attachment = (String, &'static str, String);

/// One case: `(label, template body, expected attachments as (owner, key, value))`.
type Case<'a> = (&'a str, &'a str, &'a [(&'a str, &'static str, &'a str)]);

/// Every attached comment in the template.
fn attachments(src: &str) -> Vec<Attachment> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
    let json = tsv_debug::json::wire_value(&tsv_svelte::convert_ast_json_bytes(&ast, src));
    let mut out = Vec::new();
    collect(json.get("fragment").expect("root has a fragment"), &mut out);
    out.sort();
    out.dedup();
    out
}

fn collect(node: &Value, out: &mut Vec<Attachment>) {
    match node {
        Value::Object(fields) => {
            let owner = fields
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_owned();
            for key in ["leadingComments", "trailingComments"] {
                for comment in fields
                    .get(key)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let value = comment
                        .get("value")
                        .and_then(Value::as_str)
                        .expect("comment value");
                    out.push((owner.clone(), key, value.to_owned()));
                }
            }
            for (key, value) in fields {
                if key != "leadingComments" && key != "trailingComments" {
                    collect(value, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect(i, out)),
        _ => {}
    }
}

fn check(cases: &[Case<'_>]) {
    for &(label, body, expected) in cases {
        let src = format!("{HEAD}{body}\n");
        let expected: Vec<Attachment> = expected
            .iter()
            .map(|&(owner, key, value)| (owner.to_owned(), key, value.to_owned()))
            .collect();
        assert_eq!(attachments(&src), expected, "{label}: {body:?}");
    }
}

/// An own-line comment before the pattern's closing bracket is unclaimed by the pattern
/// parse, and the annotation parse never saw it — so it attaches nowhere, on every host.
#[test]
fn a_pattern_comment_never_leads_the_annotation() {
    check(&[
        (
            "then-shorthand object, line",
            "{#await p then { a\n// c\n}: T}x{/await}",
            &[],
        ),
        (
            "then branch array, line",
            "{#await p}x{:then [a\n// c\n]: T}y{/await}",
            &[],
        ),
        (
            "catch branch object, block",
            "{#await p}x{:catch { e\n/* c */ }: E}y{/await}",
            &[],
        ),
        (
            "nested pattern: the comment closes the inner one",
            "{#await p then { a: { b\n// c\n} }: T}x{/await}",
            &[],
        ),
        ("each", "{#each xs as { a\n// c\n}: T}x{/each}", &[]),
        (
            "each with an index",
            "{#each xs as { a\n// c\n}: T, i}x{/each}",
            &[],
        ),
        (
            "each with a key",
            "{#each xs as { a\n// c\n}: T (a)}x{/each}",
            &[],
        ),
        (
            "const",
            "{#each xs as x}{@const { a\n// c\n}: T = x}{a}{/each}",
            &[],
        ),
        (
            "a claimed comment ahead of it keeps its claim",
            "{#await p then { a // c1\n/* c2 */ }: T}x{/await}",
            &[("Property", "trailingComments", " c1")],
        ),
        (
            "the annotation's own comment still leads inside the annotation",
            "{#await p then { a\n// c1\n}: /* c2 */ T}x{/await}",
            &[("TSTypeReference", "leadingComments", " c2 ")],
        ),
        (
            "the same, on each",
            "{#each xs as { a\n// c1\n}: /* c2 */ T}x{/each}",
            &[("TSTypeReference", "leadingComments", " c2 ")],
        ),
    ]);
}

/// An own-line comment the annotation parse leaves unclaimed does not fall back to trail the
/// pattern root: the fallback belongs to the discarded `_ as` wrapper, and the comment starts
/// inside it.
#[test]
fn an_annotation_comment_never_trails_the_pattern() {
    check(&[(
        "object type in the annotation",
        "{#await p then { a }: { b: T\n// c\n}}x{/await}",
        &[],
    )]);
}

/// The controls: shapes one window and two answer the same way, pinned so the split cannot
/// move them.
#[test]
fn the_controls_attach_as_before() {
    check(&[
        (
            "same-line line comment trails the entry",
            "{#await p then { a // c\n}: T}x{/await}",
            &[("Property", "trailingComments", " c")],
        ),
        (
            "same-line block comment trails the entry",
            "{#await p then { a /* c */ }: T}x{/await}",
            &[("Property", "trailingComments", " c ")],
        ),
        (
            "no annotation: the own-line comment attaches nowhere",
            "{#await p then { a\n// c\n}}x{/await}",
            &[],
        ),
        (
            "annotation comment on a destructure",
            "{#await p then [a]: /* c */ T}x{/await}",
            &[("TSTypeReference", "leadingComments", " c ")],
        ),
        (
            "annotation comment on an identifier",
            "{#await p then a: /* c */ T}x{/await}",
            &[("TSTypeReference", "leadingComments", " c ")],
        ),
    ]);
}
