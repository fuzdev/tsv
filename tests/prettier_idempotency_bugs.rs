// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Tests for cases where prettier has idempotency bugs but our printer is correct.
//!
//! These tests demonstrate that:
//! 1. Prettier requires multiple format passes to reach stable output (idempotency bug)
//! 2. Our printer produces correct output in a single pass
//!
//! See: tests/fixtures/svelte/elements/space_after_block_prettier_divergence/README.md

// Every sidecar (prettier) check is grouped into ONE `#[tokio::test]`
// (`sidecar_prettier_checks` below) for a single sequential pass over the shared,
// process-wide Deno sidecar pool. This is organizational, not a correctness
// requirement: the pool's actor tasks live on a dedicated process-lifetime runtime
// (see `deno::actor::sidecar_runtime`), so it stays safe across any number of
// separate test runtimes. Each check is a plain `async fn` the single test awaits.

async fn check_space_after_block_idempotency_bug() {
    // Prettier preserves leading space after block element on first pass,
    // but removes it on second pass (non-idempotent behavior)
    assert_prettier_idempotency_bug(
        r"<div><div>block</div> text</div>",
        "<div>\n\t<div>block</div>\n\t text\n</div>\n", // First pass (preserves space)
        "<div>\n\t<div>block</div>\n\ttext\n</div>\n",  // Second pass (removes space)
    )
    .await;
}

/// Asserts that prettier has an idempotency bug and our printer doesn't.
///
/// # Arguments
/// * `input` - Original source code
/// * `prettier_first_pass` - What prettier produces on first format (buggy output)
/// * `stable_output` - What prettier produces on second format (stable output)
///
/// # Verifies
/// 1. Prettier's first pass produces buggy output
/// 2. Prettier's second pass produces stable output (demonstrating non-idempotency)
/// 3. Our printer produces stable output in a single pass
/// 4. Our printer is idempotent
async fn assert_prettier_idempotency_bug(
    input: &str,
    prettier_first_pass: &str,
    stable_output: &str,
) {
    // Verify prettier's idempotency bug
    let prettier_once = format_with_prettier(input).await;
    assert_eq!(
        prettier_once, prettier_first_pass,
        "Prettier first pass should match expected buggy output"
    );

    let prettier_twice = format_with_prettier(&prettier_once).await;
    assert_eq!(
        prettier_twice, stable_output,
        "Prettier second pass should produce stable output"
    );

    assert_ne!(
        prettier_once, prettier_twice,
        "Prettier should not be idempotent (this demonstrates the bug)"
    );

    // Verify our printer produces stable output in one pass
    let arena = bumpalo::Bump::new();
    let our_ast = tsv_svelte::parse(input, &arena).expect("parse failed");
    let our_output = tsv_svelte::format(&our_ast, input);

    assert_eq!(
        our_output, stable_output,
        "Our printer should produce stable output in one pass"
    );

    // Verify our printer is idempotent
    let arena_twice = bumpalo::Bump::new();
    let our_ast_twice = tsv_svelte::parse(&our_output, &arena_twice).expect("parse failed");
    let our_output_twice = tsv_svelte::format(&our_ast_twice, &our_output);
    assert_eq!(
        our_output, our_output_twice,
        "Our printer should be idempotent"
    );
}

async fn format_with_prettier(content: &str) -> String {
    tsv_debug::deno::run_prettier(content, tsv_debug::deno::PrettierParser::Parser("svelte"))
        .await
        .expect("prettier formatting failed")
}

// ───────────────────────────────────────────────────────────────────────────
// Comment-drop guards on non-idempotent no-semi input
//
// A `;` keyword statement (`debugger`, the no-arg `return`, and a label-less
// `break`/`continue`) can swallow a comment that sits between the keyword and an
// explicit `;` terminator: the `;` is the statement's terminator (debugger/break/
// continue have no `[no LineTerminator]` issue once the operand/label is absent;
// the no-arg `return` reaches a following explicit `;` after ASI), so the comment
// legitimately lands *inside* the statement span. These can't be fixtures: prettier
// rewrites the input (inserting the ASI `;`, relocating the comment), so no idempotent
// `input.*` round-trips, and the diff is token-level, not whitespace-only (so it isn't
// a valid `unformatted_*` variant either). We assert the formatter output directly,
// and cross-check that prettier (with tsv's options) produces the same output —
// confirming the comment is real content, not something either tool may drop.
//
// Two failure modes guarded here: dropping the comment entirely (the old debugger
// arm), and merging consecutive own-line comments onto one line (`// c1 // c2`, the
// swallow — the old label-less break/continue arm). Both arms now route through the
// shared `split_separator_gap_comments` (own-line aware, blank line preserved).

/// `(label, input, expected)` for the comment-drop guards. Each `input` is a
/// non-idempotent no-semi form (prettier rewrites it, so it can't be a fixture);
/// `expected` is the fixed output with the interior comment preserved.
const COMMENT_DROP_CASES: &[(&str, &str, &str)] = &[
    // `// 11` sits between `debugger` and its explicit `;`; blank line preserved.
    (
        "debugger own-line + blank",
        "debugger\n\n// 11\n;[]",
        "debugger;\n\n// 11\n[];\n",
    ),
    (
        "debugger own-line, no blank",
        "debugger\n// c\n;[]",
        "debugger;\n// c\n[];\n",
    ),
    // Same-line block trails *after* the `;` (pure structure — prettier 3.9).
    (
        "debugger same-line block",
        "debugger /* c */ ;",
        "debugger; /* c */\n",
    ),
    // Same-line line floats after the `;` via `line_suffix`.
    (
        "debugger same-line line",
        "debugger // c\n;[]",
        "debugger; // c\n[];\n",
    ),
    // Two own-line comments stay on separate lines (guards the swallow direction —
    // inline emission would merge `// c2` into `// c1`'s line as text).
    (
        "debugger consecutive own-line",
        "debugger\n// c1\n// c2\n;[]",
        "debugger;\n// c1\n// c2\n[];\n",
    ),
    // The no-arg `return` reaches the explicit `;` after ASI, so `// c` is interior.
    (
        "return no-arg own-line + blank",
        "function f() {\nreturn\n\n// c\n;\n}\n",
        "function f() {\n\treturn;\n\n\t// c\n}\n",
    ),
    // A label-less `break` swallows the explicit `;`; two own-line comments between
    // them used to merge onto one line (`break; // c1 // c2`).
    (
        "break no-label consecutive own-line",
        "while (x) {\nbreak\n// c1\n// c2\n;\n}\n",
        "while (x) {\n\tbreak;\n\t// c1\n\t// c2\n}\n",
    ),
    (
        "continue no-label own-line + blank",
        "while (x) {\ncontinue\n\n// c\n;\n}\n",
        "while (x) {\n\tcontinue;\n\n\t// c\n}\n",
    ),
];

/// Pure-Rust guard: each case formats to its `expected` (the interior comment
/// survives — no drop, no swallow) and the output is idempotent. No sidecar.
#[test]
fn comment_drop_exact_output_and_idempotent() {
    for &(label, input, expected) in COMMENT_DROP_CASES {
        let arena = bumpalo::Bump::new();
        let program = tsv_ts::parse(input, &arena).expect("parse failed");
        let ours = tsv_ts::format(&program, input);
        assert_eq!(
            ours, expected,
            "case `{label}`: output should preserve the interior comment"
        );

        let arena_twice = bumpalo::Bump::new();
        let program_twice = tsv_ts::parse(&ours, &arena_twice).expect("reparse failed");
        let ours_twice = tsv_ts::format(&program_twice, &ours);
        assert_eq!(
            ours_twice, ours,
            "case `{label}`: output should be idempotent"
        );
    }
}

/// The single sidecar-using test (see the note above
/// `check_space_after_block_…`): every prettier oracle check runs here,
/// sequentially — the Svelte space-after-block idempotency-bug probe plus the
/// oracle cross-check that prettier (with tsv's options) produces the same output
/// as tsv for every typescript comment case (drop + separator-normalization),
/// confirming each comment is real content tsv must match (mirrors
/// `deno::tests::test_deno_tools`). Grouping them in one test is an organizational
/// choice for a single sequential sidecar pass — the shared sidecar pool is itself
/// safe to use from any number of separate `#[tokio::test]`s, because its actor
/// tasks live on a dedicated process-lifetime runtime, not on the per-test one.
#[tokio::test]
async fn sidecar_prettier_checks() {
    check_space_after_block_idempotency_bug().await;

    let cases = COMMENT_DROP_CASES
        .iter()
        .chain(SEPARATOR_NORMALIZED_COMMENT_CASES);
    for &(label, input, expected) in cases {
        let prettier = tsv_debug::deno::run_prettier(
            input,
            tsv_debug::deno::PrettierParser::Parser("typescript"),
        )
        .await
        .expect("prettier formatting failed");
        assert_eq!(prettier, expected, "case `{label}`: prettier should agree");
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Comment placement around a normalized type-member separator
//
// A type member's separator may be `;`, `,`, or absent (newline/ASI) — in a type
// literal (`type T = { … }`) AND an interface body (`interface I { … }`). tsv
// always emits `;`, so a `,`-separated or newline-separated member with a gap
// comment is non-idempotent input — prettier rewrites the separator, so the diff
// is token-level (not whitespace-only), ruling out both an `input.*` fixture and
// an `unformatted_*` variant. The bug: keying the partition only on `;` placed a
// comment that followed a `,` (or no separator) on the wrong side of the emitted
// `;` (`a: 1 /* c */; b: 2` instead of prettier's `a: 1; /* c */ b: 2`). The type
// literal and the interface use separate printers; both must partition on `,`/`;`.

/// `(label, input, expected)` — each `input` is a non-idempotent type member list
/// (prettier normalizes the separator to `;`); `expected` is the fixed output with
/// the comment on prettier's side of the `;`.
const SEPARATOR_NORMALIZED_COMMENT_CASES: &[(&str, &str, &str)] = &[
    // ── type literal ──
    // `,` separator: the comment follows the comma, so it leads the next member.
    (
        "comma separator",
        "type T = { a: 1, /* c */ b: 2 };",
        "type T = { a: 1; /* c */ b: 2 };\n",
    ),
    // No separator, comment on its own line leading the next member.
    (
        "newline separator, leads next",
        "type T = { a: 1\n/* c */ b: 2 };",
        "type T = { a: 1; /* c */ b: 2 };\n",
    ),
    // No separator, comment trailing the previous member on its line; the
    // synthesized `;` still goes right after the member, so the comment follows it.
    (
        "newline separator, trails prev line",
        "type T = { a: 1 /* c */\nb: 2 };",
        "type T = { a: 1; /* c */ b: 2 };\n",
    ),
    // ── interface body (separate printer; always expands multiline) ──
    // `,` separator after a property: the comment leads the next member.
    (
        "interface comma separator",
        "interface I { a: 1, /* c */ b: 2 }",
        "interface I {\n\ta: 1;\n\t/* c */ b: 2;\n}\n",
    ),
    // `,` after an index signature.
    (
        "interface index-signature comma",
        "interface I { [k: string]: number, /* c */ b: 2 }",
        "interface I {\n\t[k: string]: number;\n\t/* c */ b: 2;\n}\n",
    ),
    // `,` after a method signature.
    (
        "interface method comma",
        "interface I { m(): void, /* c */ b: 2 }",
        "interface I {\n\tm(): void;\n\t/* c */ b: 2;\n}\n",
    ),
    // No separator, comment leads the next member (already correct — regression guard).
    (
        "interface newline, leads next",
        "interface I { a: 1\n/* c */ b: 2 }",
        "interface I {\n\ta: 1;\n\t/* c */ b: 2;\n}\n",
    ),
    // No separator, comment trails the previous member's line before the synthesized
    // `;` (already correct — regression guard).
    (
        "interface newline, trails prev line",
        "interface I { a: 1 /* c */\nb: 2 }",
        "interface I {\n\ta: 1; /* c */\n\tb: 2;\n}\n",
    ),
];

/// Pure-Rust guard: each case formats to its `expected` (comment on the correct
/// side of the normalized `;`) and the output is idempotent. No sidecar.
#[test]
fn separator_normalized_comment_exact_output_and_idempotent() {
    for &(label, input, expected) in SEPARATOR_NORMALIZED_COMMENT_CASES {
        let arena = bumpalo::Bump::new();
        let program = tsv_ts::parse(input, &arena).expect("parse failed");
        let ours = tsv_ts::format(&program, input);
        assert_eq!(
            ours, expected,
            "case `{label}`: comment on the wrong side of `;`"
        );

        let arena_twice = bumpalo::Bump::new();
        let program_twice = tsv_ts::parse(&ours, &arena_twice).expect("reparse failed");
        let ours_twice = tsv_ts::format(&program_twice, &ours);
        assert_eq!(
            ours_twice, ours,
            "case `{label}`: output should be idempotent"
        );
    }
}
