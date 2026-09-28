# range_end_space_follower_long_prettier_divergence

The print-width boundary of a **space** after a root range's end marker. The space is the
follower's own per-width wrap, as after any comment
([range_end_space_follower](../range_end_space_follower_prettier_divergence/)), so it holds while
the line fits and breaks past it:

1. **A component at exactly 100** — it stays on the marker's line. Prettier breaks it at any
   width, so it splits this one too.
2. **A component at 101** — it takes its own line, under both formatters.
3. **A text at exactly 100** — it stays on the marker's line, under both formatters.
4. **A text at 101** — it moves to a fresh line. Prettier leaves a space-spelled text on the
   marker's line and lets the line run past the print width.

- `unformatted_ours_joined.svelte` — cases 2 and 4 spelled with a space: tsv breaks both back to
  `input.svelte`; prettier breaks the component and keeps the text, overflowing.
- `divergent_variant_overflow.svelte` — prettier's own form (`output_prettier.svelte` with the
  overflowing text line): prettier keeps it, tsv breaks the text and keeps the component's
  authored line, landing on `output_prettier.svelte`.

A line break after a comment is authorship, so the newline spelling of case 1 is kept by both
formatters as written (`output_prettier.svelte`).

## Reason

Design choice — the comment rule at its width boundary, identical to the one the ordinary comment
already takes ([fill_after_comment_spaced_long](../../../elements/fill_after_comment_spaced_long_prettier_divergence/)):
the space is render-free, so spending it on a break at 101 is sound, and tsv treats print width as
a hard limit where prettier overflows. Case 1 is the space the author wrote before a component,
kept while it fits
([conformance_prettier_svelte.md §What the rule does not reshape, and where convergence stops](../../../../../../docs/conformance_prettier_svelte.md#what-the-rule-does-not-reshape-and-where-convergence-stops),
[conformance_prettier.md §Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy)).

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
