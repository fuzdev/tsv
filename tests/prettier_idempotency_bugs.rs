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
