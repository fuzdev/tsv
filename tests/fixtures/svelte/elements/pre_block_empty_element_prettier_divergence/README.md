# pre_block_empty_element_prettier_divergence

An empty block-body element inside `<pre>` (white-space significant) that overflows. tsv
wraps it **lean** — when the attributes fit they stay on one line and only the close token
moves to a shallow-indented next line — and **preserves the source close form**: a
self-closing `<Comp />` / `<span … />` keeps `/>`, an explicit-empty `<Comp></Comp>` /
`<span…></span>` keeps its close tag. This holds for both components and HTML inline
elements.

Prettier (`output_prettier.svelte`) diverges: it full-breaks every attribute for the
self-closing elements (also keeping `/>`), and for the explicit-empty elements keeps the
attributes on one line but indents the wrapped `>` one level deeper. tsv injects less
whitespace into the rendered `<pre>` in every case.

When the attributes are themselves too long to fit on one line (the final pair of blocks),
both formatters wrap them one per line and the self-closing `/>` drops to its own line in
both — so the only divergence there is tsv's one-level-shallower indent. The `/>` never
hugs the last attribute (it shares the element's outer group, like every other
self-closing tag), and the explicit-empty close tag still hugs.

## Reason

`<pre>` content is whitespace-significant, so tsv minimizes injected indentation and never
rewrites the author's close form — a self-closing tag stays self-closing. See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [elements/pre_block_element_body_indent](../pre_block_element_body_indent_prettier_divergence/) — the wrapped-`>` indent divergence with an element that has children
- [elements/pre_block_body_long](../pre_block_body_long/) — the `<pre>` gate: a block text body does not drop
