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
- **The content boundary's spelling does not select the tags' layout.** Boundary whitespace is
  render-free, so the hugged, spaced and one-sided-newline authorings of one document converge;
  a newline on **both** boundaries is the author's air and is preserved instead (the
  boundary-axis paragraph below) — see
  [§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

The last four cases pin the interior side of the second rule's predicate. What lets those
newlines reflow rather than expand the element is that the content is a **fill to reflow
into** — asked of the RUN, never of the separator's shape or
the sibling count. A separator carrying words (`text1 text2`) is as much a fill as a space-only one,
and a lone element beside prose is one too, so all of them must reach the same form the
space-separated twin already reaches. Otherwise one document has two layouts depending on
whether the author happened to put words between the siblings.

The predicate's other conjunct is a **whitespace seam** to reflow at, and the negative is pinned
next door: glued content (`<a>{expr}text</a>`) is a single unbreakable unit with nothing to
reflow, so there the boundary *is* the only signal and the authored lines stay — see
[inline_multiline_nontext](../inline_multiline_nontext/), where prettier agrees.

Together they leave one form for the *inter-sibling* axis: the flowed content, boundaries hugged.
Prettier does not converge it, so it keeps the dangled form for the inter-sibling newline. The
**boundary** axis is not converged by either formatter — an authored boundary newline is the
author's air and both keep it (`variant_boundary_newline`, dual-stable; see
[inline_boundary_air](../inline_boundary_air/)), so the divergence here is the inter-sibling
separator alone.

What this fixture adds beyond those two is that the composition must settle in **one pass**. The
flow and the collapse it enables are one decision; deciding the layout against the pre-flow
content and the flow against the pre-collapse layout makes the first pass emit the block-style
form and only the second reach the inline one — so the formatter's own output is not a fixed
point (F1), and the authorings of one document land on two forms.

## Cases

An HTML inline element and a table cell (inline-classified, so it takes the same layout), then
the rest of the fill class: a component pair and an expression-tag pair whose only separator
carries words, a lone element with prose leading and trailing, and an element pair followed by
prose — the case whose newline lands in a **whitespace-only** separator node rather than inside
a content text. That last one is a distinct arm, not a restatement: where the other cases carry
their newline in a text that also holds words, this one carries it in a node that holds nothing
else, which is the one place the run's fill answer had to be consulted separately. Each is in
the converged
inline form, with the inter-sibling-newline authoring as
`unformatted_ours_newline.svelte` (tsv normalizes it to `input`; prettier instead dangles the tag
delimiters around it, a form it keeps stable — pinned as `prettier_variant_dangle.svelte`) and the
boundary-newline authoring as `variant_boundary_newline.svelte` — **dual-stable**: it is the
same document asking for air on the content boundaries, and both formatters preserve it.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
