# declaration_glued_before_prettier_divergence

A frozen declaration tag — `{let}` / `{const}` / `{#snippet}` at the root, `{@const}` inside a
component and a block body — whose directive is glued to the text before it, with whitespace
between the tag and the text after it. The freeze pins the tag's bytes; the boundary after it is
the printer's, and tsv gives it the form the same tag gets unfrozen — a line break. The directive
stays glued in front, so the tag keeps the line it shares with the text before it. An element
after the tag is the control that already takes that break.

Prettier keeps each spelling of that whitespace as its own stable form — the line break of
`input.svelte`, and the space of the variant below. tsv converges them on the break.

- `prettier_variant_space_after.svelte` — every tag followed by a space: prettier keeps it, tsv
  normalizes it to `input.svelte`.
- `unformatted_ours_tab_after.svelte` — every tag followed by a tab: tsv normalizes it to
  `input.svelte`, prettier to the space.

All of them render identically.

## Reason

Design choice and convergence, render-free under Svelte 5. A declaration tag renders nothing and
the compiler hoists it out of the fragment, so the text on its two sides meets across the
whitespace after it: `text1<!-- prettier-ignore -->{let a = 1} text2` renders `text1 text2`, and
a line break there renders the same one space. That is what licenses the tag's own line unfrozen,
and the break after it is the one a frozen tag owes too. The whitespace is never deleted:
`…{let a = 1}text2` renders `text1text2`.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
and
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [global_glued_before](../global_glued_before_prettier_divergence/) — the same boundary after a
  frozen global `svelte:*` element
