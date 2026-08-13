# boundary_air_one_sided_prettier_divergence

Air on **one** boundary. Where both content boundaries carry a newline every container kind
preserves the air alike ([inline_boundary_air](../inline_boundary_air/)); asked of a single
boundary each kind answers by its boundary rule alone:

| container | one-boundary answer | rule |
| --- | --- | --- |
| inline element (`<span>`) | collapses | both-or-neither |
| component (`<Comp>`) | collapses | both-or-neither |
| block (`<p>`), leading | expands | the leading boundary alone is the signal |
| block (`<p>`), trailing | collapses | the trailing boundary alone is not |

Where the newline **lands** is not part of the question. A leading boundary in front of text
sits inside that text node's edge run, while the same boundary in front of an element is a
whitespace-only node — one boundary, two node shapes, one answer. The component pair pins that
(cases 5 and 6), and the block pair pins the arity from both sides (leading expands, trailing
collapses — cases 3 and 4).

`unformatted_ours_one_sided.svelte` carries all seven one-boundary authorings and tsv
normalizes them to `input` in one pass — which is what makes the collapse and the expansion
each a single fixed point rather than an authoring-dependent pair.

## Prettier's forms

Prettier agrees with `input` itself — the per-kind arity is not the divergence, and on the
block-trailing and component-trailing authorings prettier normalizes to `input` too. What
differs is the **normalization** of the remaining one-sided authorings: prettier keeps its own
stable form (`prettier_variant_one_sided.svelte`), in two shapes this section already catalogs
elsewhere:

- it converts a collapsed boundary newline into a render-free **space** it keeps
  (`<span> text1 …`, `… text2 </span>`) where tsv trims the boundary;
- it keeps the leading newline and **dangles the closing delimiter** (`</Comp⏎>`) where tsv
  collapses the content inline.

So both formatters hold `input` stable and only tsv converges every one-sided authoring onto
it. Every boundary tsv trims is render-free under Svelte 5, so the output renders identically
to the input.

## Reason

Design choice: tsv converges the one-sided authorings of a document onto the same fixed point
both formatters already hold, rather than preserving prettier's boundary space and delimiter
dangle. The per-kind arity itself is shared with prettier.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
