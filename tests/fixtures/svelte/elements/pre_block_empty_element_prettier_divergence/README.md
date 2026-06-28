# pre_block_empty_element_prettier_divergence

An empty block-body element inside `<pre>` (white-space significant) that overflows. tsv
indents the wrapped attributes and close token off the element's nesting depth — one level
per enclosing container, the same model as prettier — and preserves the source close form:
a self-closing `<Comp />` / `<span … />` keeps `/>`, an explicit-empty `<Comp></Comp>` /
`<span…></span>` keeps its close tag.

The divergence is **print-width-as-hard-limit**: for a self-closing element whose attributes
fit within print width (counting the preserved-text prefix already on the line), tsv keeps
them on one line and moves only `/>` to its own line, whereas prettier
(`output_prettier.svelte`) full-breaks every attribute regardless. The explicit-empty cases
(close token on its own line, attributes inline) and the genuinely-overflowing cases
(attributes wrapped one per line, `/>` on its own line) now match prettier byte-for-byte —
they remain here as contrast. The `/>` never hugs the last attribute (it shares the
element's outer group, like every other self-closing tag), and the explicit-empty close tag
still hugs.

## Reason

`<pre>` content is whitespace-significant, so tsv treats print width as a hard limit and
keeps attributes inline when the real line fits, injecting less whitespace into the rendered
`<pre>`. It never rewrites the author's close form — a self-closing tag stays self-closing.
See [conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [elements/pre_nested_attr_indent](../pre_nested_attr_indent/) — the depth-accumulating attribute indent inside `<pre>` (non-divergence: tsv now matches prettier)
- [elements/pre_block_body_long](../pre_block_body_long/) — the `<pre>` gate: a block text body does not drop
