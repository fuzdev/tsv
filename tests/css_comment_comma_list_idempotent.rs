// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A multi-item comma-separated declaration value that carries a comment at the
//! list's **top level** breaks one-per-line — the same, in one pass, for every
//! render-equivalent authoring of that value (comment glued to a neighbour, or
//! spaced away from it). This matches prettier, which force-breaks such a list.
//!
//! Regression this guards (an F1 fixed-point violation): the printer decided the
//! break from the parsed value *structure*, which flips with the whitespace around
//! the comment — a space adjacent to a top-level comment makes the value parser
//! split it into a spurious space-separated `List` element, and only a `List`
//! triggered the one-per-line break. A glued authoring (`x,/* c */y`) has no such
//! space, so it stayed inline on pass 1; the value normalizer then *inserts* a
//! space after the comment's `*/` (`x, /* c */ y`), so pass 2 saw the `List` and
//! broke — the format was not a fixed point. Keying the break on the presence of a
//! top-level comment (not on the incidental `List`) makes every authoring reach the
//! one broken form in a single pass. See CLAUDE.md §Comment Handling and the
//! `css-normalize-value-text-context-blind` bug class.
//!
//! A comment **nested inside a function argument** (`var(--a, /* c */ red)`) sits
//! below the list's top level and does NOT force the break — prettier keeps it
//! inline, and so must tsv (cases 4/5 below).
//!
//! Not a fixture: pinning this in the fixture corpus would add a directory, which
//! reshuffles the `fuzz:audit` seed-0 sample onto the next latent bug (the exact
//! hazard this regression surfaced from) — so it lives here, with the live prettier
//! oracle kept via `sidecar_prettier_agrees` below.

/// `(label, input, expected, has_prettier_oracle)`. Each `input` formats to
/// `expected` in one pass and `expected` is a fixed point. For every case with
/// `has_prettier_oracle`, prettier (with tsv's options) also produces `expected`
/// — re-verified live by `sidecar_prettier_agrees`. The malformed mutant (case 6)
/// has no oracle: prettier rejects it (`Unbalanced parenthesis`), so it guards
/// idempotency only, not prettier parity.
const CASES: &[(&str, &str, &str, bool)] = &[
    // ── the break (glued top-level comment) ──
    // Minimal valid trigger: a comment glued to the second comma element.
    (
        "glued minimal",
        "a{color: x,/* c */y}",
        "a {\n\tcolor:\n\t\tx,\n\t\t/* c */ y;\n}\n",
        true,
    ),
    // A comment glued to the *first* element trails it on its own broken line.
    (
        "glued after first element",
        "a{font-family: Arial/* x */, sans-serif}",
        "a {\n\tfont-family:\n\t\tArial /* x */,\n\t\tsans-serif;\n}\n",
        true,
    ),
    // The spaced authoring of the same value reaches the identical fixed point
    // (it already broke via the spurious `List`; this pins that it stays put).
    (
        "spaced authoring — same fixed point",
        "a{color: x, /* c */ y}",
        "a {\n\tcolor:\n\t\tx,\n\t\t/* c */ y;\n}\n",
        true,
    ),
    // ── no break (comment is not at the list's top level) ──
    // Nested inside `var(...)`: prettier keeps this inline, so must tsv.
    (
        "nested function comment stays inline",
        "a{color: var(--a, /* c */ red), blue}",
        "a {\n\tcolor: var(--a, /* c */ red), blue;\n}\n",
        true,
    ),
    // A comma *inside* the comment is not a list separator — the value is a single
    // leaf, never a comma list, and stays inline.
    (
        "comma inside comment is not a list",
        "a{color: /* x, y */ red}",
        "a {\n\tcolor: /* x, y */ red;\n}\n",
        true,
    ),
    // ── the actual fuzz mutant (idempotency-only; prettier rejects it) ──
    // `var,` (not `var(`) is a top-level comma element; the trailing `)` is a stray
    // token the mutation left behind. No prettier oracle — the invariant under test
    // is the fixed point, which holds regardless.
    (
        "fuzz mutant: top-level comma list, glued comment, stray paren",
        "a{color: var,--a,/* comment */red)}",
        "a {\n\tcolor:\n\t\tvar,\n\t\t--a,\n\t\t/* comment */ red);\n}\n",
        false,
    ),
];

fn format_css(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let stylesheet = tsv_css::parse(source, &arena).expect("parse failed");
    tsv_css::format(&stylesheet, source)
}

/// Each input formats to its expected form in one pass, and that form is a fixed
/// point (formats to itself). The one-pass assertion is what fails on the buggy
/// glued authorings today (they format inline first, break only on the second
/// pass); the fixed-point assertion is the F1 invariant the regression violated.
#[test]
fn comment_comma_list_one_pass_break_and_idempotent() {
    for &(label, input, expected, _) in CASES {
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

/// Live oracle: prettier (with tsv's options) produces the same `expected` form,
/// confirming each break/no-break decision matches prettier rather than a value
/// baked in by hand. The malformed mutant is skipped — prettier rejects it. One
/// `#[tokio::test]` for a single sequential pass over the shared sidecar pool
/// (mirrors `prettier_idempotency_bugs.rs`).
#[tokio::test]
async fn sidecar_prettier_agrees() {
    for &(label, input, expected, has_oracle) in CASES {
        if !has_oracle {
            continue;
        }
        let prettier =
            tsv_debug::deno::run_prettier(input, tsv_debug::deno::PrettierParser::Parser("css"))
                .await
                .expect("prettier formatting failed");
        assert_eq!(prettier, expected, "case `{label}`: prettier should agree");
    }
}
