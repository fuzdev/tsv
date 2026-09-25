# global_glued_both_sides_wide_long_prettier_divergence

A global `svelte:*` element glued to text on **both** sides, in a line too wide to fit. The
compiler hoists `<svelte:window>` / `<svelte:document>` / `<svelte:body>` / `<svelte:head>` out of
the fragment before its whitespace rules run, so the text on either side meets the other directly
(`text1text3`): neither glued boundary may take a break, and the element lays out as an inline
element glued on both sides does.

- **`<svelte:head>` with content, past the dangle → block-style.** The dangle line
  (`text1<svelte:head>…</svelte:head`) would be 101, one past the widest that still takes the
  closing-`>` dangle
  ([global_glued_both_sides_head_dangle_bound_long](../global_glued_both_sides_head_dangle_bound_long/)
  holds it at exactly 100), so tsv moves the content to its own indented line between the two
  intact tags, both still glued to the text. Prettier **double-dangles**
  (`<svelte:head⏎\t>…</svelte:head⏎>`).
- **Inside a paragraph → the welded unit travels.** `text18<svelte:window …/>text19` is one
  unbreakable unit, and the whitespace in front of it is the one break available, so tsv moves the
  whole unit to a fresh line. Prettier keeps it on the text line and wraps the element's
  attributes instead. Two cases: a unit far past the width, and one that would end its line at
  exactly 101 — the twin of the 100-wide unit that stays on its line under both formatters in
  [global_glued_both_sides_long](../global_glued_both_sides_long/).

`output_prettier.svelte` is prettier's output from `input.svelte` (it keeps the block-style head
and wraps the paragraph's element). `prettier_variant_dangle.svelte` is prettier's stable form of
the one-line authoring `unformatted_ours_compact.svelte`; tsv normalizes both to `input.svelte`.
All of them render identically — the content boundaries inside `<svelte:head>` are trimmed at
compile, the break before the unit is inter-node whitespace, and the glued boundaries are never
split.

## Reason

Design choice, render-free under Svelte 5 — the two bounds an inline element glued on both sides
already takes: block-style past the dangle, as in
[inline_glued_before_long](../../elements/inline_glued_before_long_prettier_divergence/), and the
welded-run travel of
[inline_welded_run_travel_long](../../elements/inline_welded_run_travel_long_prettier_divergence/).
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
