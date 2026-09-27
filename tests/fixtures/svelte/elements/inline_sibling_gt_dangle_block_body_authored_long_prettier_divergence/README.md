# inline_sibling_gt_dangle_block_body_authored_long_prettier_divergence

An inline element or component holding an `{#await}`, or a `{#snippet}` glued to content on both
sides, whose body breaks for any reason **other than structure**: a break inside an element in
the body (a child holding an `{#if}`, a child whose attribute value spans lines), a multi-line
comment, width, or the author's own line breaks around a section — an `{#await}` section, a
`{:catch}` section, a snippet body. The element's content lays out block-style and, like its
width-broken twin, it hands its `>` on in both sibling-`>` roles: onto a glued multiline block
(`</b⏎>{#if cond}`), onto a glued element whose attributes wrap (`</Comp⏎><i⏎\ttitle=…`), and
from a glued first element (`<i>text</i⏎\t><b⏎\t\ttitle=…`).

The body's line breaks are what the one-line authoring prints, so they cannot be the signal: an
authoring that already has them decides as the one that does not. The block is transparent down
its own sections but stops at an element, so an `{#if}` inside an `<i>` in the body is that
`<i>`'s structure, not the element's. The width cases pin the 100/101 boundary of
the `{#await}` on its own line: at 100 it stays inline there and at 101 its body drops, and the
element sheds its `>` on both sides. Two controls shed their `>` as well: a section holding one
block element, which does not force the body open, and the same content with no block around it.

- `unformatted_ours_compact.svelte` — every case on one line but for the authored line breaks the
  sections-on-their-own-lines cases name. tsv normalizes it to `input.svelte`; prettier to
  `prettier_variant_hugged.svelte`.
- `unformatted_ours_kept_gt.svelte` — every `>` kept on its closing tag (`</b>{#if cond}`,
  `</Comp><i`, `<i>text</i><b`) — the block-style layout with the `>` hugged. A deliberate copy
  of `output_prettier.svelte`, carrying the claim that tsv normalizes it to `input.svelte`.
- `prettier_variant_hugged.svelte` — prettier's form of the compact authoring: every element's
  delimiters dangle around content it keeps welded to the block
  (`<b⏎\t>{#await promise}text{:then value}<i⏎\t\t\t>{#if cond}{value}{/if}</i⏎\t\t>{/await}</b⏎>{#if cond}…`).
  Prettier-stable; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier's form of `input.svelte`: the same layout with every `>`
  kept on its closing tag.

Every form renders the same: the `>` moves only inside an end tag, and the forms otherwise differ
only in whitespace at content boundaries, which is not rendered.

## Reason

Design choice. Which elements take part in the sibling-`>` dangle is decided by *cause*, and an
element's block-style output is byte-for-byte the authored-multiline form, a block body's section
breaks included: a role keyed on those breaks would be re-decided on the next pass. Only structure
keeps the `>`. Prettier keeps it on every closing tag. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
(Sibling `>` dangle) and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).

## Related

- [elements/inline_sibling_gt_dangle_block_body_structure](../inline_sibling_gt_dangle_block_body_structure_prettier_divergence/) — a block body that breaks by its own content: the element keeps its `>`
- [elements/inline_sibling_gt_dangle_shedder_long](../inline_sibling_gt_dangle_shedder_long_prettier_divergence/) — the shedder whose own content breaks by width or authoring
- [elements/inline_sibling_gt_dangle_inline_parent](../inline_sibling_gt_dangle_inline_parent_prettier_divergence/) — an element before an `{#await}` whose body breaks
