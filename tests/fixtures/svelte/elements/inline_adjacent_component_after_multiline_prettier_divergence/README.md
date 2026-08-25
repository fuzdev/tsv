# inline_adjacent_component_after_multiline_prettier_divergence

The adjacent separator before a component whose predecessor renders **multiline** — a block-styled
`<span>` here — spelled as a **space**. tsv hugs the component onto the predecessor's closing tag
(`</span> <Comp />`), exactly as it does for an inline element in that position: the boundary is
measured from the closing tag's column, where the component fits. Prettier's `isInlineElement`
admits only a `RegularElement`, so it splits the component off onto its own line:
`output_prettier.svelte` is that own-line form.

The **newline** spelling of the same boundary is held by **both** formatters — the layout-keyed
rule [sibling_newline_after_multiline](../sibling_newline_after_multiline_prettier_divergence/)
pins, for every inline sibling kind — so the boundary is dual-stable here: `variant_newline.svelte`
is the own-line form, which is also what prettier makes of the input. The
`unformatted_ours_glued_head.svelte` variant glues `text1` to the container's opening tag; tsv
normalizes it to the input, prettier does not.

The cases vary only what made the container multiline — the run and its separators are the same in
each — so the component's boundary does not depend on the container's cause. The prose-free control
keeps its authored line: with no fill to reflow into, the newline is the author's only structure,
and both formatters hold it.

## Reason

Design choice: an authored space is a hug in every sibling kind — the component takes the
inline-sibling wrap an element takes, in every arm that prints this separator — where prettier
keys the answer on the sibling's kind.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
