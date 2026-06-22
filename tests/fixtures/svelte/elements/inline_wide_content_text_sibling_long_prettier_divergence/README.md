# inline_wide_content_text_sibling_long_prettier_divergence

The companion to `inline_wide_content_trailing_long`: a wide inline element whose **content**
overflows, but here the following text (`mid`) is **non-terminal** — another inline element
(`<b>`) follows it.

tsv lays the element out **block-style** (both tags intact, the over-wide content wrapped within
printWidth — prettier keeps it on one over-width dangled line), and the text run between the two
elements takes its **own line**. This is the **contrast** with the terminal case
(`inline_wide_content_trailing_long`), which hugs a space-authored tail onto the closing-tag line:
non-terminal text **must not** hug, because hugging it is non-convergent — placing `mid` on the
closing line shifts where the following `<b>` lands, which feeds back into the fit decision (a
flip-flop across passes). So this fixture is the guard that the boundary-respecting hug stays scoped
to *terminal* trailing text. The `mid <b>x</b>` line matches prettier, so the divergence is purely
the block-style content layout.

The `unformatted_ours_*` variants pin idempotence: the single-line and multiline authorings both
normalize to the same own-line form in one pass.

## Reason

A wide inline element's content lays out block-style to honor printWidth (prettier keeps it inline
and dangles); a *non-terminal* text run (one followed by another flowing element) takes its own
line, because hugging it onto the element's closing line is non-convergent. (The *terminal* case
instead respects the author's space and hugs — see `inline_wide_content_trailing_long`.)

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
