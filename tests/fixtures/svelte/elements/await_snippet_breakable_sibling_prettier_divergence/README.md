# await_snippet_breakable_sibling_prettier_divergence

A short `{#await}` / `{#snippet}` immediately after a **breakable** sibling — an inline
element (`<span>`), an expression tag (`{e}`), `{@html}`, or `{@render}` — inside a
**block element** (`<div>`): tsv drops the element's children to their own line (the
parent goes multiline), while prettier keeps an inline-authored construct inline.

- **tsv** force-breaks the block-element parent so the construct's body-drop and the
  inline-element sibling-`>` dangle resolve in one pass. The block itself stays hugged to
  its sibling — when it is short there is no `>` to dangle — so only the parent breaks.
- **prettier** is whitespace-preserving here: it keeps the inline-authored form inline
  (and would keep an authored-broken form broken). It never expands the block.

`prettier_variant_inline.svelte` is the inline-authored form: tsv normalizes it to
`input.svelte` (the parent breaks), but prettier keeps it inline (stable), so prettier
does **not** normalize it to `input.svelte` — which is also why there is no
`output_prettier.svelte` (prettier keeps `input.svelte` itself stable too, being
whitespace-preserving).

## Reason

`{#await}` / `{#snippet}` don't force their parent multiline on their own, but once they
follow a **breakable** sibling (the `is_inline_content` set, which sets
`has_preceding_breakable`), tsv routes the parent through the multiline layout so the
body-drop / dangle / block-sibling separation all resolve in one pass — at the cost of
breaking the parent for the short case, where prettier stays inline. A **non-breakable**
sibling (plain text, a comment) does not trigger this: tsv keeps it inline, matching
prettier (see [elements/await_snippet_nonbreakable_sibling_inline](../await_snippet_nonbreakable_sibling_inline/)).
See [conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [elements/await_snippet_nonbreakable_sibling_inline](../await_snippet_nonbreakable_sibling_inline/) — the non-breakable (text / comment) counterpart that stays inline
- [elements/inline_sibling_gt_dangle](../inline_sibling_gt_dangle_prettier_divergence/) — the same breakable inline-element sibling when the block renders multiline (the `>` dangles)
- [blocks/await/preceding_sibling_body_long](../../blocks/await/preceding_sibling_body_long_prettier_divergence/) — await body-drop after an expression-tag sibling
