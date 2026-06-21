# inline_wide_content_text_sibling_long_prettier_divergence

The companion to `inline_wide_content_trailing_long`: a wide inline element whose **content**
overflows, but here the following text (`mid`) is **non-terminal** — another inline element
(`<b>`) follows it.

tsv wraps the over-wide content within printWidth (the hard-limit divergence — prettier keeps it
on one over-width line, see `output_prettier.svelte`), and the text run between the two elements
takes its **own line** after the dangled closing `>`. This is the **contrast** with the terminal
case (`inline_wide_content_trailing_long`), which now hugs a space-authored tail: non-terminal text
**must not** hug, because hugging it is non-convergent — placing `mid` on the dangled-`>` line
shifts where the following `<b>` lands, which feeds back into the fit decision (a flip-flop across
passes). So this fixture is the guard that the boundary-respecting hug stays scoped to *terminal*
trailing text. The `mid <b>x</b>` line matches prettier, so the divergence is purely the content
wrap.

The `unformatted_ours_*` variants pin idempotence: the single-line and multiline authorings both
normalize to the same own-line form in one pass.

## Reason

A wide inline element's content wraps to honor printWidth (prettier keeps it inline); a
*non-terminal* text run (one followed by another flowing element) takes its own line, because
hugging it onto the wrapped element's last line is non-convergent. (The *terminal* case instead
respects the author's space and hugs — see `inline_wide_content_trailing_long`.) See
[conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
