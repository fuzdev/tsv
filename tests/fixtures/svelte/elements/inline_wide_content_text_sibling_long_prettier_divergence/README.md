# inline_wide_content_text_sibling_long_prettier_divergence

The companion to `inline_wide_content_trailing_long`: a wide inline element whose **content**
overflows, but here the following text (`mid`) is **non-terminal** — another inline element
(`<b>`) follows it.

tsv wraps the over-wide content within printWidth (the hard-limit divergence — prettier keeps it
on one over-width line, see `output_prettier.svelte`), and the text run between the two elements
takes its **own line** after the dangled closing `>`. This is the **same uniform rule** as the
terminal case (`inline_wide_content_trailing_long`): tsv never hugs trailing text onto a wrapped
element's last line. The added following element exercises the trailing-`line` boundary (text
before another flowing child); the `mid <b>x</b>` line matches prettier, so the divergence is
purely the content wrap.

The `unformatted_ours_*` variants pin idempotence: the single-line and multiline authorings both
normalize to the same own-line form in one pass.

## Reason

A wide inline element's content wraps to honor printWidth (prettier keeps it inline); trailing
text — terminal or, as here, before a following element — always takes its own line under tsv's
uniform never-hug rule. See
[conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
