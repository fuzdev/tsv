# title_boundary_whitespace_prettier_divergence

A `<title>` outside `<svelte:head>` parses as a **RegularElement**, so its fits-inline
content-boundary whitespace follows the general Svelte-mirror trim: the compiler deletes
every fragment-edge run at compile (`clean_nodes`), so `<title> text </title>` renders
identically to `<title>text</title>` and tsv trims it — bare at the root and nested
inside a regular element alike.

Prettier preserves the authored boundary space on the regular form (the HTML/CSS inline
whitespace model), so the spaced authoring is a prettier-stable form tsv normalizes:

- `prettier_variant_spaces.svelte` — prettier's stable spaced forms; tsv normalizes both
  cases to the glued `input.svelte`.

The contrast is the **TitleElement** form — `<title>` as a direct/transparent child of
`<svelte:head>` — where prettier trims the boundary spaces too, so only the regular form
diverges. That agreement is pinned by
[`special_elements/title_in_head`](../../special_elements/title_in_head/)'s
`unformatted_spaces.svelte` (its RegularElement case is deliberately left unpadded there,
because this fixture owns the divergence).

## Reason

Same class as
[inline_boundary_whitespace](../inline_boundary_whitespace_prettier_divergence/) —
pinned separately because prettier's `<title>` handling (metadata content,
`display: none`) could change independently of ordinary inline elements. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
