# block_after_spaced_text_prettier_divergence

**A block element after spaced text takes its own line.** A block sibling partitions the
inline run and its own break separates the two, so the space before it is spent on that break
in every fragment family — a block element, a section, an inline element, a component, a block
body and the root — and whether the block renders inline (`text1 text2⏎<div>block1</div>`) or
multiline (its head never ends the text line). Prettier keeps the block hugged to the text when
the text is the fragment's **first child** (`text1 text2 <div>block1</div>`,
`prettier_variant_space.svelte`) and breaks it after any other predecessor; tsv normalizes the
space spelling to `input.svelte` in one pass.

The controls agree with prettier: a text that is *not* the fragment's first child (after an
element, after a tag) breaks before the block, an element predecessor does, and a **glued**
boundary is split the same way (`unformatted_glued.svelte` — a block boundary is render-free,
so both formatters normalize it to input).

## Reason

Design choice — the last row-dependent cell of the space rule. A space between siblings is
decided by the follower's kind alone, and a block-element follower breaks it after an element, a
component, a tag, a comment, a `<br />` and a control-flow block — and after a text that is not
the fragment's first child. Only a **first-child** text kept the block on its line, and only in
the space spelling: the glued and newline spellings of the same boundary already break. That is
a spelling and a position selecting a layout, not a policy: prettier's hug is its
`printChildren` returning early for a first child before the block-follower trim runs, and tsv
had mirrored it. The multiline block is the sharper case — a multiline unit's head never ends a
content line for an inline element, a component or a control-flow block, and the block element
was the one unit kind still dangling its head there.

Prettier's own HTML printer breaks every one of these shapes as tsv does, and the newline form is
what the default display renders: a `<div>` amid inline content is a block box on its own line.
The cost falls on a `<div>` restyled inline (a flex item, `display: inline`), which the
default-display model excludes — there a `<span>` hugs. The same early return, at the text's other
edge, is [space_after_block](../space_after_block_prettier_divergence/)'s stray leading space.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
