// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A parenthesized Svelte expression ISLAND ROOT — a JSDoc cast `/** @type {T} */ (x)` or a
//! bare grouping paren `(x)` — never receives Svelte's leftover-comment fallback.
//!
//! Svelte parses every template expression with `preserveParens: true`
//! (`svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js`), so acorn's comment walk
//! sees a `ParenthesizedExpression` root. The walk makes one trailing claim — on the inner
//! expression, when the gap to the comment is in acorn's `/^[,) \t]*$/` class — and the
//! post-walk fallback then hands every comment still left past the root's end to the ROOT,
//! which is the paren wrapper. `remove_parens` discards that wrapper with its
//! `trailingComments`, so at a parenthesized root:
//!
//! - a trailing RUN keeps only its first comment (`(x) /* a */ /* b */` → `x: [a]`);
//! - a comment past a newline after the `)` attaches nowhere (the gap is not in the class,
//!   and the fallback that would have caught it is thrown away).
//!
//! An unparenthesized root keeps the fallback (`x /* a */ /* b */` → `x: [a, b]`), and so
//! does a root whose parens are interior to it (`(r)() /* a */ /* b */`, a call).
//!
//! **Why a test rather than a fixture.** The format-stable cast shapes are fixtured in
//! [template_expr_paren_trailing_comment](../tests/fixtures/svelte/syntax/comments/template_expr_paren_trailing_comment_prettier_divergence/)
//! and repeated here so every position reads in one place. The rest can't be: a newline
//! injected before the comment is not a fixed point of either formatter, and neither is a
//! bare grouping paren (both print `{x /* c */}`).
//!
//! The pair's LEADING side is the same discard: a comment before the `(` — the cast's own
//! `@type` comment, or any comment before a bare root pair — leads the
//! `ParenthesizedExpression` and dies with it
//! (`a_comment_before_a_root_pair_attaches_nowhere`). Every expectation is transcribed
//! from the live modern parser (`cargo run -p tsv_debug canonical_parse`), so it would go
//! stale silently if Svelte changed.

use serde_json::Value;

/// One trailing attachment: `(owner path from the root, comment values in order)`.
type Attachment = (String, Vec<String>);

/// One case: `(template, expected attachments as (owner path, values))`.
type Case<'a> = (&'a str, &'a [(&'a str, &'a [&'a str])]);

const TRAILING: &str = "trailingComments";
const LEADING: &str = "leadingComments";

/// Every `list` (`trailingComments` / `leadingComments`) in the template, keyed by its
/// owner's path.
fn attachments(src: &str, list: &str) -> Vec<Attachment> {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("component should parse");
    let json = tsv_debug::json::wire_value(&tsv_svelte::convert_ast_json_bytes(&ast, src));
    let mut out = Vec::new();
    collect(
        json.get("fragment").expect("root has a fragment"),
        "fragment",
        list,
        &mut out,
    );
    out
}

fn collect(node: &Value, path: &str, list: &str, out: &mut Vec<Attachment>) {
    match node {
        Value::Object(fields) => {
            for (key, value) in fields {
                if key == list {
                    let values = value
                        .as_array()
                        .expect("a comment list is an array")
                        .iter()
                        .map(|c| {
                            c.get("value")
                                .and_then(Value::as_str)
                                .expect("comment value")
                                .to_owned()
                        })
                        .collect();
                    out.push((path.to_owned(), values));
                } else if key != TRAILING && key != LEADING {
                    collect(value, &format!("{path}.{key}"), list, out);
                }
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                collect(item, &format!("{path}[{i}]"), list, out);
            }
        }
        _ => {}
    }
}

fn check(cases: &[Case<'_>]) {
    check_list(cases, TRAILING);
}

fn check_list(cases: &[Case<'_>], list: &str) {
    let mut failures = Vec::new();
    for &(src, expected) in cases {
        let expected: Vec<Attachment> = expected
            .iter()
            .map(|&(path, values)| {
                (
                    path.to_owned(),
                    values.iter().map(|&v| v.to_owned()).collect(),
                )
            })
            .collect();
        let actual = attachments(src, list);
        if actual != expected {
            failures.push(format!(
                "{src:?}\n    tsv:       {actual:?}\n    canonical: {expected:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} case(s) differ from the canonical parser ({list}):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// A run of trailing comments past a cast island root: only the first attaches, at every
/// position a template expression can be the root.
#[test]
fn a_run_past_a_cast_root_keeps_only_its_first_comment() {
    check(&[
        (
            "{@html /** @type {C} */ (x) /* a */ /* b */}",
            &[("fragment.nodes[0].expression", &[" a "])],
        ),
        (
            "{#if /** @type {B} */ (x) // a\n// b\n}\n\tyes\n{/if}",
            &[("fragment.nodes[0].test", &[" a"])],
        ),
        (
            "<div title={/** @type {A} */ (x) /* a */ /* b */}>text</div>",
            &[("fragment.nodes[0].attributes[0].value.expression", &[" a "])],
        ),
        (
            "<button onclick={/** @type {F} */ (h) /* a */ /* b */}>go</button>",
            &[("fragment.nodes[0].attributes[0].value.expression", &[" a "])],
        ),
        (
            "<input bind:value={/** @type {V} */ (v) /* a */ /* b */} />",
            &[("fragment.nodes[0].attributes[0].expression", &[" a "])],
        ),
        (
            "<div {.../** @type {P} */ (p) /* a */ /* b */}></div>",
            &[("fragment.nodes[0].attributes[0].expression", &[" a "])],
        ),
        (
            "{@render /** @type {R} */ (r()) /* a */ /* b */}",
            &[("fragment.nodes[0].expression", &[" a "])],
        ),
        (
            "{#each /** @type {L} */ (xs) /* a */ /* b */ as x}y{/each}",
            &[("fragment.nodes[0].expression", &[" a "])],
        ),
        (
            "{#if true}{@const a = /** @type {K} */ (x) /* a */ /* b */}{/if}",
            &[(
                "fragment.nodes[0].consequent.nodes[0].declaration.declarations[0].init",
                &[" a "],
            )],
        ),
        (
            "{/** @type {E} */ (x) /* a */ /* b */}",
            &[("fragment.nodes[0].expression", &[" a "])],
        ),
        (
            "{#key /** @type {K} */ (x) /* a */ /* b */}y{/key}",
            &[("fragment.nodes[0].expression", &[" a "])],
        ),
        (
            "{#await /** @type {W} */ (p) /* a */ /* b */}y{/await}",
            &[("fragment.nodes[0].expression", &[" a "])],
        ),
    ]);
}

/// A comment past a newline after a parenthesized root's `)` is outside acorn's trailing gap
/// class, and the fallback that would catch it went away with the wrapper: no attachment.
#[test]
fn a_comment_past_a_newline_after_a_paren_root_attaches_nowhere() {
    check(&[
        ("{#if /** @type {B} */ (x)\n\t /* t2 */}yes{/if}", &[]),
        ("{@html /** @type {C} */ (x)\n /* t3 */}", &[]),
        // `c1` sits inside the parens past a newline (no walk claim), `c2` past the `)` — the
        // wrapper's, discarded
        ("{#if (a\n\t // c1\n) // c2\n}t{/if}", &[]),
        ("{(x)\n/* c */}", &[]),
        ("{#if (a)\n// c\n}t{/if}", &[]),
    ]);
}

/// A bare grouping paren is the same root as a cast: its first trailing comment attaches to
/// the inner expression (the `)` is in the gap class), and a run keeps only that one.
#[test]
fn a_bare_paren_root_takes_only_the_first_trailing_comment() {
    check(&[
        (
            "{(x) /* c */}",
            &[("fragment.nodes[0].expression", &[" c "])],
        ),
        (
            "{#if true}{@const a = (x) /* p */}{/if}",
            &[(
                "fragment.nodes[0].consequent.nodes[0].declaration.declarations[0].init",
                &[" p "],
            )],
        ),
        (
            "<div title={(x) /* c */}>t</div>",
            &[("fragment.nodes[0].attributes[0].value.expression", &[" c "])],
        ),
        (
            "{#if (x) /* a */ /* b */}y{/if}",
            &[("fragment.nodes[0].test", &[" a "])],
        ),
        (
            "{@render (r()) /* a */ /* b */}",
            &[("fragment.nodes[0].expression", &[" a "])],
        ),
    ]);
}

/// Shapes that already match and must keep matching: a lone comment after a cast root, a
/// claim the walk makes INSIDE the parens, a block's key expression, and the roots that are
/// not a paren wrapper and so keep the fallback.
#[test]
fn controls_that_match_the_canonical_parser() {
    check(&[
        (
            "{@html /** @type {C} */ (x) /* t3 */}",
            &[("fragment.nodes[0].expression", &[" t3 "])],
        ),
        (
            "{#if (a // c1\n) // c2\n}t{/if}",
            &[("fragment.nodes[0].test", &[" c1"])],
        ),
        (
            "{#each xs as x (key // c\n)}y{/each}",
            &[("fragment.nodes[0].key", &[" c"])],
        ),
        // the root is the CALL, whose parens are interior: the fallback takes the whole run
        (
            "{@render /** @type {R} */ (r)() /* a */ /* b */}",
            &[("fragment.nodes[0].expression", &[" a ", " b "])],
        ),
        (
            "{@render (r)() /* a */ /* b */}",
            &[("fragment.nodes[0].expression", &[" a ", " b "])],
        ),
        // unparenthesized roots: the fallback takes the run, and a comment past a newline
        (
            "{@html x /* a */ /* b */}",
            &[("fragment.nodes[0].expression", &[" a ", " b "])],
        ),
        (
            "{@html x\n /* t3 */}",
            &[("fragment.nodes[0].expression", &[" t3 "])],
        ),
        (
            "{#if a\n\t// c1\n}t{/if}",
            &[("fragment.nodes[0].test", &[" c1"])],
        ),
    ]);
}

/// The pair's leading side: a comment before a root pair's `(` — the cast's own `@type`
/// comment, or any comment ahead of a bare pair — leads the `ParenthesizedExpression`,
/// which `remove_parens` discards with it, so it attaches nowhere. A cast whose pair is
/// not the root keeps its comment on the node that starts at the `(` and survives.
#[test]
fn a_comment_before_a_root_pair_attaches_nowhere() {
    check_list(
        &[
            ("{@html /** @type {C} */ (x)}", &[]),
            ("<div title={/** @type {A} */ (x)}>t</div>", &[]),
            ("{#if true}{@const a = /** @type {K} */ (x)}{/if}", &[]),
            ("{/* c */ (x)}", &[]),
            ("{( /* c */ (x))}", &[]),
            ("{#if /* c */ (a)}t{/if}", &[]),
            // a non-root cast in a call argument: its pair is the argument, and leads
            ("{f(/** @type {T} */ (a), b)}", &[]),
            // the cast's pair starts the call, so the comment leads the CALL and survives
            (
                "{@render /** @type {R} */ (r)()}",
                &[("fragment.nodes[0].expression", &["* @type {R} "])],
            ),
            // the member's span starts at the `(`, so the member takes it
            (
                "{#if /** @type {T} */ (a).b}t{/if}",
                &[("fragment.nodes[0].test", &["* @type {T} "])],
            ),
            // a `{let}` tag is a `parse_statement_at` parse with no `preserveParens`
            (
                "{let a = /** @type {T} */ (x)}",
                &[(
                    "fragment.nodes[0].declaration.declarations[0].init",
                    &["* @type {T} "],
                )],
            ),
        ],
        LEADING,
    );
}
