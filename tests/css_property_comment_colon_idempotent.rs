// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A CSS declaration whose **property name carries a block comment**
//! (`color /* c */ : value`) takes tsv's normalized separator — single spaces
//! around the comment, and a **space before the colon** (` : `) — the same,
//! regardless of the value's kind. See the sanctioned form in fixture
//! `css/tokens/comments/in_property_value_before_colon_prettier_divergence`
//! (tsv normalizes; prettier preserves the input spacing and glues the property
//! to the comment — `color/* c */ : red`).
//!
//! Regression this guards (an F1 fixed-point violation): the property→colon
//! separator was decided **per value-kind dispatch path**, and only the
//! single-value path (`print_decl_default`) emitted the ` : ` (space before the
//! colon). Every other path — comma list, space list, string, function, value
//! comment, grid — emitted `: ` (no space before the colon), a
//! symmetric-position inconsistency: `color/* c */: red` normalized to
//! `color /* c */ : red;` while `color/* c */: red, blue` normalized to
//! `color /* c */: red, blue;`.
//!
//! That inconsistency became a **non-idempotency** on a value whose kind flips
//! across passes: a leading-comma value (`color /* c */ : ,ed`) parses as a
//! comma list on pass 1 (routed to the `: ` path, the comma then dropped), and
//! its formatted output `color /* c */: ed;` reparses as a single identifier
//! on pass 2 (routed to the ` : ` path) — so the separator oscillated and the
//! format was not a fixed point. Hoisting the "property carries a comment ⇒
//! space before the colon" decision to one predicate, emitted once before the
//! value-kind dispatch, makes every kind agree in a single pass. See CLAUDE.md
//! §Comment Handling and the `css-normalize-value-text-context-blind` bug class.
//!
//! Not a fixture: pinning idempotency in the fixture corpus would add a
//! directory, which reshuffles the `fuzz:audit` seed-0 sample onto the next
//! latent bug (the exact hazard this regression surfaced from) — so it lives
//! here, with the live prettier divergence kept via `sidecar_prettier_diverges`
//! below. The single-value sanctioned form is already pinned by the
//! `in_property_value_before_colon` fixture; this test extends the guarantee to
//! every value kind and to the malformed value-kind-flip mutant.

/// `(label, input, expected)`. Each `input` formats to `expected` in one pass,
/// and `expected` is a fixed point (formats to itself). The two malformed
/// leading-comma mutants (the fuzz findings) have no prettier oracle — prettier
/// keeps the leading comma as an empty broken element and tsv drops it — so they
/// guard idempotency only. The valid cases diverge from prettier solely by the
/// documented property↔comment glue (see `PRETTIER_DIVERGENCE` below).
const CASES: &[(&str, &str, &str)] = &[
    // ── the fuzz non-idempotency mutants (malformed; idempotency-only) ──
    // A leading-comma value parses as a comma list, whose formatted output drops
    // the comma and reparses as a single identifier — the value-kind flip that
    // oscillated the separator.
    (
        "malformed leading-comma value (seed-123 fuzz non-idempotency)",
        "a{color/* comment */ : ,ed}",
        "a {\n\tcolor /* comment */ : ed;\n}\n",
    ),
    // The same flip with a colon *inside* the comment (scan robustness — the real
    // `property : value` colon is the one outside the comment).
    (
        "malformed leading-comma value, colon inside the comment",
        "a{color/* x:y */ :, red}",
        "a {\n\tcolor /* x:y */ : red;\n}\n",
    ),
    // ── valid values: the separator is ` : ` for every kind ──
    // A genuine comma list — the inconsistency case (was `: `, no space before
    // the colon). Now matches the single-value form.
    (
        "valid comma list",
        "a{color/* c */: red, blue}",
        "a {\n\tcolor /* c */ : red, blue;\n}\n",
    ),
    // A single value — already correct; the regression guard that the hoist did
    // not change the path that was right.
    (
        "valid single value (regression guard)",
        "a{color/* c */: red}",
        "a {\n\tcolor /* c */ : red;\n}\n",
    ),
    // A string value (`print_decl_string`).
    (
        "valid string value",
        "a{color/* c */: \"x\"}",
        "a {\n\tcolor /* c */ : 'x';\n}\n",
    ),
    // A function value (`print_decl_function`).
    (
        "valid function value",
        "a{width/* c */: calc(1px + 2px)}",
        "a {\n\twidth /* c */ : calc(1px + 2px);\n}\n",
    ),
    // A space-separated list value (`print_decl_value_list` List arm).
    (
        "valid space-separated list value",
        "a{margin/* c */: 1px 2px}",
        "a {\n\tmargin /* c */ : 1px 2px;\n}\n",
    ),
];

/// `(label, input, prettier_expected)`. For the valid cases whose input is
/// already in tsv's canonical spacing except the property↔comment glue, the
/// *only* live divergence from prettier is that glue: tsv writes
/// `color /* c */ : …`, prettier writes `color/* c */ : …`. Pinning prettier's
/// exact output keeps the divergence honest (it is exactly the glue, and
/// prettier agrees on the ` : ` space before the colon).
const PRETTIER_DIVERGENCE: &[(&str, &str, &str)] = &[
    (
        "comma list — divergence is the property↔comment glue",
        "a{color /* c */ : red, blue}",
        "a {\n\tcolor/* c */ : red, blue;\n}\n",
    ),
    (
        "single value — divergence is the property↔comment glue",
        "a{color /* c */ : red}",
        "a {\n\tcolor/* c */ : red;\n}\n",
    ),
];

fn format_css(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let stylesheet = tsv_css::parse(source, &arena).expect("parse failed");
    tsv_css::format(&stylesheet, source)
}

/// Each input formats to its expected form in one pass, and that form is a fixed
/// point (formats to itself). The one-pass assertion fails today on the malformed
/// mutants (they oscillate) and on every non-single-value valid case (they emit
/// `: ` rather than ` : `); the fixed-point assertion is the F1 invariant the
/// regression violated.
#[test]
fn property_comment_colon_one_pass_and_idempotent() {
    for &(label, input, expected) in CASES {
        let out = format_css(input);
        assert_eq!(
            out, expected,
            "case `{label}`: input should format to the expected form in one pass"
        );

        let out_twice = format_css(expected);
        assert_eq!(
            out_twice, expected,
            "case `{label}`: expected form must be a fixed point (idempotent)"
        );
    }
}

/// Live oracle: prettier (with tsv's options) diverges from tsv only by the
/// documented property↔comment glue — it agrees on the ` : ` space before the
/// colon. One `#[tokio::test]` for a single sequential pass over the shared
/// sidecar pool (mirrors `css_comment_comma_list_idempotent.rs`).
#[tokio::test]
async fn sidecar_prettier_diverges() {
    for &(label, input, prettier_expected) in PRETTIER_DIVERGENCE {
        let prettier =
            tsv_debug::deno::run_prettier(input, tsv_debug::deno::PrettierParser::Parser("css"))
                .await
                .expect("prettier formatting failed");
        assert_eq!(
            prettier, prettier_expected,
            "case `{label}`: prettier should produce the documented divergent form"
        );
        // tsv normalizes the glue to a space; that is the *only* difference.
        assert_eq!(
            format_css(input),
            prettier_expected.replacen("color/* c */", "color /* c */", 1),
            "case `{label}`: tsv should differ from prettier only by the property↔comment glue"
        );
    }
}
