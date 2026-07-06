// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Parser pins for the contextual-type-keyword qualified-name HEAD (`string.X`)
//! — the deliberate tsv-rejects/acorn-accepts divergence the fixture path can't
//! reach (an `input_invalid_*` fixture requires BOTH parsers to reject; here
//! acorn-typescript accepts). Cataloged in `docs/conformance_svelte.md`
//! §Reserved-keyword qualified type head.
//!
//! A type keyword immediately followed by `.` is the HEAD of a qualified type
//! name. tsv qualifies it only for the *contextual* type keywords
//! (`string`/`number`/`any`/`undefined`/…) — matching tsc AND prettier — and
//! rejects the *reserved* `void`/`null` heads, which acorn-typescript
//! (over-permissively) accepts as a `TSQualifiedName`. `true`/`false` are
//! literal types in every parser, so `true.X` rejects on both sides
//! (fixture-pinned as `input_invalid_true_qualified_head`, not here). The accept
//! direction is fixture-pinned (`types/type_keyword_qualified_head`); the
//! boundary case below documents the contrast in one place.

/// tsv parses + formats standalone-TS `input` to itself, then re-formats
/// stably (idempotent).
fn assert_ours_stable(input: &str) {
    let arena = bumpalo::Bump::new();
    let ast = tsv_ts::parse(input, &arena).expect("parse failed");
    let output = tsv_ts::format(&ast, input);
    assert_eq!(output, input, "printer should keep the form stable");

    let arena_twice = bumpalo::Bump::new();
    let ast_twice = tsv_ts::parse(&output, &arena_twice).expect("reparse failed");
    let output_twice = tsv_ts::format(&ast_twice, &output);
    assert_eq!(output, output_twice, "printer should be idempotent");
}

fn assert_ours_rejects(input: &str) {
    let arena = bumpalo::Bump::new();
    assert!(
        tsv_ts::parse(input, &arena).is_err(),
        "expected parse rejection (pinned divergence): {input}"
    );
}

/// The reserved `void`/`null` heads are rejected — tsv follows tsc/prettier,
/// not acorn-typescript's over-permissive `TSQualifiedName` reading.
#[test]
fn reserved_keyword_qualified_head_rejected() {
    assert_ours_rejects("let a: void.X;");
    assert_ours_rejects("let a: null.X;");
    assert_ours_rejects("type T = void.A.B;");
}

/// Boundary: the *contextual* type keywords DO qualify (matching acorn AND
/// tsc) — only the reserved heads diverge.
#[test]
fn contextual_keyword_qualified_head_stable() {
    assert_ours_stable("let a: string.X;\n");
    assert_ours_stable("let a: undefined.M;\n");
    assert_ours_stable("type T = number.A.B;\n");
}
