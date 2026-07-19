# pre_block_empty_element_prettier_divergence

An empty block-body element inside `<pre>` (white-space significant) that overflows. tsv
indents the wrapped attributes and close token off the element's nesting depth — one level
per enclosing container, the same model as prettier.

The close form follows tsv's one rule, not a `<pre>`-local one: a **component** keeps an
authored `/>` (`<Comp />` stays self-closing), while a plain **HTML** element normalizes to its
close tag — so the `<span>` case is `></span>` whichever form the author wrote.
`unformatted_ours_selfclosing_span.svelte` is the self-closing authoring, which tsv normalizes
to `input.svelte`; prettier instead preserves the `/>` *and* full-breaks the attributes, pinned
as `prettier_variant_selfclosing_span.svelte`. See
[elements/ws_sensitive_self_closing_kinds](../ws_sensitive_self_closing_kinds_prettier_divergence/) for the full
kind × attributes matrix and [elements/pre_void_element](../pre_void_element/) for the void
case.

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
`<pre>`. That is a claim about *content*: whitespace-sensitivity governs how much whitespace
the layout injects, not how a tag serializes. The close form is therefore decided by the same
`can_self_close` rule as everywhere else — rewriting `<span … />` to `<span …></span>` adds no
characters to the rendered `<pre>`.
See [conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [elements/pre_nested_attr_indent](../pre_nested_attr_indent/) — the depth-accumulating attribute indent inside `<pre>` (non-divergence: tsv now matches prettier)
- [elements/pre_block_body_long](../pre_block_body_long/) — the `<pre>` gate: a block text body does not drop
