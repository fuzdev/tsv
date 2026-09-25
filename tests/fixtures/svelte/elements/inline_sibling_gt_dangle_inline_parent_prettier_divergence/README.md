# inline_sibling_gt_dangle_inline_parent_prettier_divergence

The sibling-`>` dangle inside an **inline parent** — a component or an inline element.
An inline element glued to a following `{#await}` (or to a `{#snippet}` glued to content on
both sides) dangles its closing `>` onto the block-head line when the block renders
multiline (`</span⏎>{#await…}`), exactly as it does under a block parent
([inline_sibling_gt_dangle](../inline_sibling_gt_dangle_prettier_divergence/)).

Neither block forces an inline parent multiline on its own — a short block stays inline and
keeps the `>` hugged (the two control cases, `{#await}` and `{#snippet}`) — so the parent's
layout here is decided by the block alone. What breaks it varies per case: a nested
`<style>`, a multi-line HTML comment, a `<pre>` in a `{:then}` branch, a `<style>` in a
`{:catch}` branch, and width. Under width the `>` dangles in two ways: when the block fits on
the fresh line the dangle opens it stays inline there, and when it does not its body drops
too. The element may also be the tail of a glued run, where only the run's LAST closing `>`
dangles, and may follow text and a space (`text <span>text</span⏎>{#await…}`).

- `unformatted_ours_compact.svelte` — every case authored on one line; tsv normalizes it
  to `input.svelte`, prettier to `prettier_variant_hug.svelte`.
- `prettier_variant_hug.svelte` — prettier's form of the compact authoring: it dangles the
  PARENT's delimiters (`<Comp⏎\t>…</Comp⏎>`) where tsv lays the parent's content out
  block-style (see
  [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)),
  and keeps the block welded to its body. Prettier-stable; tsv normalizes it to
  `input.svelte`.
- `output_prettier.svelte` — prettier's form of `input.svelte`: the same layout but for
  the `>`, which prettier hugs (`</span>{#await…}`), breaking the body's own attributes
  where tsv's dangle leaves room.

The dangle moves the `>` only inside the closing tag, so it is render-safe — the dangled and
hugged forms compile to byte-identical server JS.

## Reason

The dangle keys on the rendered layout, not on the parent or on how the body is authored,
so it must be a one-pass fixed point in an inline parent too. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
(Sibling `>` dangle).

## Related

- [elements/inline_sibling_gt_dangle](../inline_sibling_gt_dangle_prettier_divergence/) — the same dangle under a block parent, all block heads
- [blocks/await/sibling_head_wrap_inline_parent_long](../../blocks/await/sibling_head_wrap_inline_parent_long_prettier_divergence/) — a long `{#await}` head after a sibling in the same inline-parent position
- [blocks/await/inline_element_long](../../blocks/await/inline_element_long_prettier_divergence/) — an `{#await}` inside an inline element with no preceding sibling
