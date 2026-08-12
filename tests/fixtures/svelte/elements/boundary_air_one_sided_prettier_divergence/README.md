# boundary_air_one_sided_prettier_divergence

Air on **one** boundary. Where both content boundaries carry a newline every container kind
preserves the air alike ([inline_boundary_air](../inline_boundary_air/)); asked of a single
boundary the kinds answer differently, and that arity is the whole rule:

| container | one-boundary answer | rule |
| --- | --- | --- |
| inline element (`<span>`) | collapses | both-or-neither |
| block (`<p>`) | expands | the leading boundary alone is the signal |
| component (`<Comp>`), content starts with **text** | expands | any newline inside a content *text* counts |
| component (`<Comp>`), content starts with an **element** | collapses | the same boundary, in a different node |

The last two are one case split by where the newline **lands**, and they are the reason this
fixture exists. A component is both-or-neither like an inline element, but it also treats a
newline anywhere inside a content text node as a break — and a leading boundary in front of text
*is* inside that text node, while a leading boundary in front of an element sits in a
whitespace-only node the content trim drops. So the pair is not an exception to the arity, it is
the arity meeting a different node shape. Written as a single "components need both boundaries"
claim the behavior reads as a bug; written as the pair, it is the rule.

`unformatted_ours_one_sided.svelte` carries all five one-boundary authorings and tsv normalizes
them to `input` in one pass — which is what makes the collapse and the expansion each a single
fixed point rather than an authoring-dependent pair.

## Prettier's forms

Prettier agrees with `input` itself — the arity is not the divergence. What differs is the
**normalization**: from the one-sided authoring prettier keeps its own stable form
(`prettier_variant_one_sided.svelte`), in two shapes this section already catalogs elsewhere:

- it converts a collapsed boundary newline into a **rendered-free space** it keeps
  (`<span> text1 …`, `… text2 </span>`) where tsv trims the boundary;
- it **dangles the closing delimiter** (`</Comp⏎>`) where tsv keeps both tags intact.

So both formatters hold `input` stable and only tsv converges the one-sided authoring onto it.
Every boundary tsv trims is render-free under Svelte 5, so the output renders identically to the
input.

## Reason

Design choice: tsv converges the one-sided authorings of a document onto the same fixed point
both formatters already hold, rather than preserving prettier's boundary space and delimiter
dangle. The per-kind arity itself is shared with prettier.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
