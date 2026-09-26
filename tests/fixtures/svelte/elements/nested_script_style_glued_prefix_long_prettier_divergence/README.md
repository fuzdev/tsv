# nested_script_style_glued_prefix_long_prettier_divergence

Text in front of a nested `<script>` glued to content on both sides, at the print-width boundary.
The glued run (`a<script>…</script>text15`) cannot take a break inside — the element renders no
box, so a break there would render a space — and it renders multiline, so tsv spends the space in
front of it on a line break and the run starts its own line, at every width. Prettier keeps the
run on the text line: at exactly 100 the line `…text14 a<script>` fits, and at 101 prettier
dangles the opening tag's `>` onto the next line (`aa<script⏎\t>`).

- `output_prettier.svelte` — prettier's output from `input.svelte`.
- `unformatted_ours_compact.svelte` — each paragraph on one line; tsv normalizes it to
  `input.svelte`, prettier to its own forms above.

All of them render identically: the space in front of the run collapses to one rendered space
whichever way it is spelled, and the glued boundaries are never split.

## Reason

Design choice, render-free under Svelte 5 — the multiline unit's drop to a fresh line of
[inline_sibling_space_before_bounding](../inline_sibling_space_before_bounding_prettier_divergence/),
the same form a glued inline element with a breaking body takes. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [nested_script_style_glued_block_style](../nested_script_style_glued_block_style_prettier_divergence/)
  — the same drop with a short line
- [nested_script_style_glued_long](../nested_script_style_glued_long/) — text after the closing
  tag, where both formatters agree
