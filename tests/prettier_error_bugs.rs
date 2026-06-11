// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

/// Tests for cases where prettier errors (`Comment "..." was not printed`)
/// but our printer formats correctly.
///
/// These cases can't be fixtures — the fixture pipeline requires prettier to
/// format `input.*`, and prettier's TS printers crash on them (the svelte
/// plugin "succeeds" only by passing the script content through unformatted).
/// Each test also asserts that prettier still fails, so a future prettier
/// version that fixes the bug flags the case for promotion into a fixture.

#[tokio::test]
async fn optional_param_comment_no_annotation() {
    // Comment between a parameter name and `?` with no type annotation.
    // Prettier crashes; ours preserves the comment before `?`.
    assert_prettier_errors_ours_stable(
        "<script lang=\"ts\">\n\tfunction fn(a /* c */?) {}\n</script>\n",
    )
    .await;
}

#[tokio::test]
async fn optional_arrow_param_comment_no_annotation() {
    assert_prettier_errors_ours_stable(
        "<script lang=\"ts\">\n\tconst fn = (a /* c */?) => {};\n</script>\n",
    )
    .await;
}

/// Asserts that prettier fails to format `input` while our printer keeps it
/// stable (formats to itself) and idempotent.
async fn assert_prettier_errors_ours_stable(input: &str) {
    let prettier_result =
        tsv_debug::deno::run_prettier(input, tsv_debug::deno::PrettierParser::Parser("svelte"))
            .await;
    assert!(
        prettier_result.is_err(),
        "Prettier should error on this input — if a prettier update fixed it, \
         promote this case into a fixture: {prettier_result:?}"
    );

    let ast = tsv_svelte::parse(input).expect("parse failed");
    let output = tsv_svelte::format(&ast, input);
    assert_eq!(output, input, "Our printer should keep the input stable");
}
