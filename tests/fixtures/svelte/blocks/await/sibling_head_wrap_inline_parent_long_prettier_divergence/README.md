# sibling_head_wrap_inline_parent_long_prettier_divergence

An `{#await}` after a **breakable** sibling inside an **inline parent** (a component or an
inline element), at the 100/101 boundary of the block-head line. The sibling is either glued
to the block — an expression tag `{expr}` or a component — or an inline element and a space,
which puts the head on its own line. At 100 the head stays on one line; at 101 it wraps and
its `}` dangles, exactly as it does under a block parent. In every case the construct is too
wide for one line, so the parent lays out block-style and the body drops.

- `unformatted_ours_compact.svelte` — every case authored on one line; tsv normalizes it
  to `input.svelte`, prettier to `prettier_variant_hug.svelte`.
- `prettier_variant_hug.svelte` — prettier's form of the compact authoring: the parent's
  delimiters dangle (`<Comp⏎\t>…</Comp⏎>`) and the block stays inline past print width.
  Prettier-stable; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier's form of `input.svelte`: prettier never wraps a
  block head, so it rejoins each wrapped head onto one line — past print width, except
  after the component, where it breaks the component's `/>` onto the head's line instead.

## Reason

A block head wraps and dangles its `}` once its line exceeds print width, whatever the
parent. An `{#await}` does not force an inline parent multiline on its own, so this is the
shape where the parent's block-style layout comes from width alone — and the head must
reach the same form in one pass. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).

## Related

- [await/inline_element_long](../inline_element_long_prettier_divergence/) — the head wrap inside an inline element with no preceding sibling
- [await/preceding_sibling_body_long](../preceding_sibling_body_long_prettier_divergence/) — a preceding `{x}` sibling under a block parent, where the BODY overflows
- [elements/inline_sibling_gt_dangle_inline_parent](../../../elements/inline_sibling_gt_dangle_inline_parent_prettier_divergence/) — the sibling-`>` dangle in the same inline-parent position
