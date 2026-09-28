# range_end_space_follower_prettier_divergence

A root `prettier-ignore-start` / `-end` range whose end marker is followed by a **space** and
then a component, an expression tag, an `{@html}` tag, an inline control-flow block, a
`<svelte:element>`, an ordinary comment or the start of another range. The end marker is an
HTML comment, and tsv lays out the boundary after it exactly as it lays out the boundary after
any comment in the same position: the space is the follower's own per-width wrap, so it stays a
space on the marker's line while the line fits. Prettier breaks each of them onto its own line
(`output_prettier.svelte`).

- `variant_newline.svelte` — the newline spelling of every boundary, which is prettier's form:
  both formatters keep it, since a line break after a comment is authorship.
- `unformatted_ours_spaces.svelte` — each space widened to three: tsv collapses them back to
  `input.svelte`, prettier breaks the line.
- `unformatted_ours_tabs.svelte` — each space spelled as a tab: tsv prints the space, prettier
  breaks the line.

Both spellings render identically. The space breaks where the same space after an ordinary
comment breaks: past the print width
([range_end_space_follower_long](../range_end_space_follower_long_prettier_divergence/)), and
before a block element, which takes its own line under both formatters
([range_end_glued_follower](../range_end_glued_follower/)). A text or an inline element after
the space stays on the marker's line under both formatters while the line fits, and an inline
follower glued to the marker stays glued under both; a block element glued to it takes its own
line under both.

## Reason

Design choice — the range's end marker takes the rule every comment already follows, rather than
a line of its own that only the marker would force. A space before an inline follower — an
expression tag, a component, an inline block, a comment — is not turned into a newline the author
did not write while the line fits; the newline spelling is what holds a comment's line
([§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
[conformance_prettier_svelte.md §What the rule does not reshape, and where convergence stops](../../../../../../docs/conformance_prettier_svelte.md#what-the-rule-does-not-reshape-and-where-convergence-stops)).
The freeze covers the bytes between the two markers, the end marker included, not the whitespace
after it, which the printer lays out as it would without the range.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
