// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

/// Tests for cases where prettier has idempotency bugs but our printer is correct.
///
/// These tests demonstrate that:
/// 1. Prettier requires multiple format passes to reach stable output (idempotency bug)
/// 2. Our printer produces correct output in a single pass
///
/// See: tests/fixtures/svelte/elements/prettier_bug_space_after_block/README.md

#[tokio::test]
async fn prettier_bug_space_after_block() {
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

/// Oracle cross-check: prettier (with tsv's options) produces the same output for
/// every case — confirming the comment is real content neither tool may drop. A
/// single `#[tokio::test]` runs the cases sequentially to avoid the shared-sidecar
/// multi-runtime race (mirrors `deno::tests::test_deno_tools`).
#[tokio::test]
async fn comment_drop_cases_match_prettier() {
    for &(label, input, expected) in COMMENT_DROP_CASES {
        let prettier = tsv_debug::deno::run_prettier(
            input,
            tsv_debug::deno::PrettierParser::Parser("typescript"),
        )
        .await
        .expect("prettier formatting failed");
        assert_eq!(prettier, expected, "case `{label}`: prettier should agree");
    }
}
