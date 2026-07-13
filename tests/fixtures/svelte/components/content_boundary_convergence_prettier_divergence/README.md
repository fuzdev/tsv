# content_boundary_convergence_prettier_divergence

The clearest statement of the block-style stance: **one document, three authorings, three stable
prettier forms, one tsv form.**

A component with wrapping attributes and a single block child. The only difference between `input`,
`prettier_variant_hug_start`, and `prettier_variant_hug_end` is whether the author put whitespace
before the child, after it, or neither — and Svelte 5 removes whitespace at the start and end of a
tag's content at compile, so all three render identically.

Prettier lets that whitespace pick the layout and so keeps **each** form stable: hug the opening `>`
to the child, or hug the child to `</Comp` and dangle its `>`, or neither. tsv treats the boundary as
carrying no signal and converges all three on the block-style form — both tags intact, content on its
own indented line — which is also the form prettier keeps stable when it is given it.

Supersedes the former `hug_start_only`, `hug_end_only`, and `hug_neither` fixtures, which pinned one
tsv output per authoring.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
