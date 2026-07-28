# inline_content_flow_collapse_prettier_divergence

tsv: an inline element whose content is a run of inline siblings converges on the **fully inline**
form — the inter-sibling newline flows back onto the content line, the hugged boundaries are
preserved, and the element collapses. Prettier: holds a distinct stable form for each authoring.

## Reason

**Design choice — the flow rule, and the layout it feeds, decided together.** Two rules compose,
and both are already sanctioned:

- **The inter-sibling newline flows.** Svelte 5 collapses an inter-sibling whitespace run to one
  space, so a newline and a space between two siblings render identically and the spelling carries
  no signal — see
  [inline_sibling_newline_flow](../inline_sibling_newline_flow_prettier_divergence/).
- **The content boundary does not select the layout.** Boundary whitespace is render-free, so the
  hugged, spaced and newline authorings of one document converge — see
  [§Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).

Together they leave exactly one form: the flowed content, boundaries hugged. Prettier converges
neither axis, so it keeps a stable form for each authoring — the dangled form for the
inter-sibling newline, and the block-style form for the boundary newline
(`prettier_variant_boundary_newline`).

What this fixture adds beyond those two is that the composition must settle in **one pass**. The
flow and the collapse it enables are one decision; deciding the layout against the pre-flow
content and the flow against the pre-collapse layout makes the first pass emit the block-style
form and only the second reach the inline one — so the formatter's own output is not a fixed
point (F1), and the authorings of one document land on two forms.

## Cases

An HTML inline element and a table cell (inline-classified, so it takes the same layout), each
in the converged inline form, with the inter-sibling-newline authoring as
`unformatted_ours_newline.svelte` (tsv normalizes it to `input`; prettier instead dangles the tag
delimiters around it, a form it keeps stable — pinned as `prettier_variant_dangle.svelte`) and the
boundary-newline authoring as `prettier_variant_boundary_newline.svelte` (prettier keeps it
stable; tsv normalizes it to `input`).

See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
