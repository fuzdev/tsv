# pre_block_element_body_indent_prettier_divergence

Inside `<pre>` (white-space is significant), the block-body drop is gated off — but an
inline-element body that overflows still wraps its own closing `>` to a new line. tsv
indents that wrapped `>` **one level shallower than prettier** (tsv `</span⏎\t>`, prettier
`</span⏎\t\t>`), injecting less whitespace into the rendered `<pre>` content.

Both formatters keep their own form stable. The body itself does **not** drop (see
`elements/pre_block_body_long`); only the element's `>` wraps.

## Reason

`<pre>` content is whitespace-significant, so tsv minimizes injected indentation — it does
not add the block's nesting level to the wrapped `>` the way prettier does. See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [elements/pre_block_body_long](../pre_block_body_long/) — the `<pre>` gate: a block body does not drop inside `<pre>`
