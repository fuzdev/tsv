# inline_sibling_gt_dangle_block_body_structure_prettier_divergence

An inline element or component holding an `{#await}`, or a `{#snippet}` glued to content on both
sides, whose body breaks **by structure**: the body's own content forces it open however it is
authored. That is the content the block itself reads as forcing its body open, in any section —
the pending, `{:then}` and `{:catch}` sections and a snippet body:

- more than one child with a block element, a block special element or a control-flow block among
  them (two block elements, text beside a block element, text beside an `{#await}`);
- a declaration on its own line;
- an authored blank line between two children.

The element is multiline by structure, so it keeps its own `>` in both sibling-`>` roles: before
a glued multiline block (`</b>{#if cond}`, `</Comp>{#each …}`), before a glued element whose
attributes wrap (`</b><i⏎\ttitle=…`), and as the second element of a glued pair
(`<i>text</i><b⏎\t\ttitle=…`). The control holds two block elements directly and keeps its `>`
too. The block's test is its own, not the element's: an element holding the same blank-separated
children or text beside an `{#await}` directly still sheds its `>`.

The block is transparent all the way down its own sections: a section that is itself a block is
read through to that block's sections, so an `{#await}` whose section is an `{#await}` holding two
block elements keeps the `>` too. It stops at an element, whose content is the element's own. A
section that is itself an `{#if}` keeps the `>` as well — an `{#if}` inside an `{#await}` makes the
element multiline by structure on its own. The rule holds with text ahead of the block in the
element, in block and inline parents, and inside an `{#each}` body.

A `prettier-ignore`d `{#await}` whose section holds two block elements makes its element multiline
by the same structure: the element lays out block-style around the frozen block, as it does around
a frozen `{#if}`.

The block body's content is read the way the block reads it, so the one-line authoring and the
block-style one decide alike: the body breaks in both, and neither sheds the `>`.

- `unformatted_ours_compact.svelte` — every case on one line but for the blank line its case
  names. tsv normalizes it to `input.svelte`; prettier to `prettier_variant_hugged.svelte`.
- `unformatted_dangled_gt.svelte` — every case with its `>` dangled onto the next line
  (`</b⏎>{#if cond}`, `</b⏎><i`, `<i>text</i⏎\t><b`), the shed form of the block-style layout.
  Both formatters normalize it to `input.svelte`.
- `prettier_variant_hugged.svelte` — prettier's form of the compact authoring: the element's
  delimiters dangle around content it keeps welded to the block
  (`<b⏎\t>{#await promise}<Comp />{:then value}<div>{value}</div>⏎\t\t<div>block1</div>{/await}</b⏎>{#if cond}…`).
  Prettier-stable; tsv normalizes it to `input.svelte`.

Prettier keeps `input.svelte` as written. Every form renders the same: the `>` moves only inside an
end tag, and the forms otherwise differ only in whitespace at content boundaries and beside block
elements, which is not rendered.

## Reason

Design choice. Which elements take part in the sibling-`>` dangle is decided by *cause*: an
element multiline by structure keeps its `>`, since structure is a cause the dangle's own output
never rewrites. A block body that breaks by its own content is structure; its line breaks are not
the signal. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
(Sibling `>` dangle) and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).

## Related

- [elements/inline_sibling_gt_dangle_block_body_authored_long](../inline_sibling_gt_dangle_block_body_authored_long_prettier_divergence/) — a block body that breaks by width, by authoring, or by a break inside an element: the element sheds its `>`
- [elements/inline_sibling_gt_dangle_shedder_long](../inline_sibling_gt_dangle_shedder_long_prettier_divergence/) — the shedder whose own content breaks
- [elements/inline_parent_block_child_await](../inline_parent_block_child_await_prettier_divergence/) — a block element beside an `{#await}`, structural too
