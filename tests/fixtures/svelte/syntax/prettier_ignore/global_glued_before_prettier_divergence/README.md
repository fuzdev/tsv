# global_glued_before_prettier_divergence

A frozen global `svelte:*` element (`<svelte:window>` / `<svelte:document>` / `<svelte:body>` /
`<svelte:head>`) whose directive is glued to the content before it, with whitespace between the
element and the text or inline element after it. The freeze pins the element's bytes; the
boundary after it is the printer's, and tsv gives it the form the same element gets unfrozen —
a line break. The directive stays glued in front, so the element keeps the line it shares with
the content before it.

Prettier keeps each spelling of that whitespace as its own stable form — the line break of
`input.svelte`, and the space of the variant below. tsv converges them on the break.

- `prettier_variant_space_after.svelte` — every element followed by a space: prettier keeps it,
  tsv normalizes it to `input.svelte`.
- `unformatted_ours_spaces.svelte` — runs of spaces and tabs after each element: tsv normalizes
  them to `input.svelte`, prettier collapses each to one space.

All of them render identically.

## Reason

Design choice and convergence, render-free under Svelte 5. The compiler hoists these four
elements out of the fragment before its whitespace rules run, so the content on the element's two
sides meets across the whitespace after it: `text1<!-- prettier-ignore --><svelte:head>…</svelte:head> text3`
renders `text1 text3`, and a line break there renders the same one space. That makes the break
after the element free, which is the break an unfrozen element glued on one side takes
([global_glued_one_side](../../../special_elements/global_glued_one_side_prettier_divergence/)),
and one form for the two spellings is the convergence tsv makes of every render-equivalent
authoring. The whitespace is never deleted: with the element hoisted, `…</svelte:head>text3`
renders `text1text3`.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
and
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [global_glued_before_neighbours](../global_glued_before_neighbours_prettier_divergence/) — the
  directive's own left neighbour does not change the boundary
- [global_space_after](../global_space_after_prettier_divergence/) — an own-line directive, a
  hoisted neighbour, a `<svelte:head>` that spans lines
- [global_glued_before_followers](../global_glued_before_followers/) — followers prettier breaks
  before too, and the glued-on-both-sides control
- [global_glued_before_long](../global_glued_before_long_prettier_divergence/) — the break is not
  part of the glued unit's width
- [inline_space_after](../inline_space_after/) — a frozen inline node keeps its space, as it does
  unfrozen
