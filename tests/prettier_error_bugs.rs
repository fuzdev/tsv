// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

/// Tests for cases where prettier errors (`Comment "..." was not printed`)
/// but our printer formats correctly.
///
/// ⚠️ These SHOULD be fixtures and are not yet. "Prettier throws on the input" is
/// exactly what the `prettier_rejects.txt` marker expresses (trimmed content = the
/// expected-error substring, in a `_prettier_divergence` dir — see
/// `docs/fixture_overview.md`), and one existing fixture already pins this very error
/// class: `typescript/class/body_prettier_ignore_empty_prettier_divergence` holds
/// `Comment "prettier-ignore" was not printed`. So the fixture pipeline does NOT
/// require prettier to succeed; the belief that it does is what kept these here.
/// Each test also asserts that prettier still fails, so a future prettier version
/// that fixes the bug flags the case for promotion — which a `prettier_rejects.txt`
/// fixture does too, and against the live oracle rather than a hand-written string.

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

#[tokio::test]
async fn static_import_source_phase() {
    // Source-phase import `import source x from '…'`. prettier's `typescript` parser
    // reads `source` as a name and throws (`'=' expected`); ours parses + keeps it
    // stable. (acorn rejecting it is not itself a fixture blocker — that is
    // representable; prettier throwing is what keeps this one here.) The
    // parser is graded by test262; see `tests/import_phase.rs` for the printer's
    // round-trip coverage and `docs/conformance_prettier_ts.md` for the catalog.
    assert_prettier_errors_ours_stable(
        "<script lang=\"ts\">\n\timport source x from 'x';\n</script>\n",
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

    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(input, &arena).expect("parse failed");
    let output = tsv_svelte::format(&ast, input);
    assert_eq!(output, input, "Our printer should keep the input stable");
}
