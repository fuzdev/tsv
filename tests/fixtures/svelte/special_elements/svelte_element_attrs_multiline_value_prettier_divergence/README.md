# svelte_element_attrs_multiline_value_prettier_divergence

`<svelte:element>` and `<slot>` whose attribute value is a multiline arrow function, so the
attributes always break.

Broken attributes put the `>` on its own line; the question is what the *content* does. tsv lays it
out **block-style** — both tags intact, content on its own indented line — while prettier hugs the
content to the `}}` of the last attribute and **dangles** the closing delimiter
(`}}>content</svelte:element⏎>`), because the source hugged the content boundary.

That hug is render-free under Svelte 5, so it carries no signal and must not select the layout. The
attribute group and the content group are independent in tsv: attributes wrap on their own width, and
content breaks on its own — which is also what keeps this shape idempotent.

`prettier_variant_dangle` is prettier's stable form (tsv normalizes it to `input`). The self-closing
case above matches prettier.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
