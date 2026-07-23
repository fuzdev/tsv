# inline_break_before_chain_long_prettier_divergence

The 3+-element chain case of the glued sibling-`>` dangle (G2), which generalizes from a pair
(`inline_break_before_gt_dangle_long`) to a whole **run** of byte-glued inline elements. Two
things the run-level treatment guarantees that a pairwise one could not:

1. **The whole run breaks before, as a unit.** Preceded by same-line text that must wrap, the
   entire glued run moves to a fresh line at the whitespace boundary before it. A wide element
   *anywhere* in the run pulls the whole run over — an earlier short element (`<span>a</span>`)
   can no longer keep the run on the text line while only the wide tail wraps. The run is
   measured flat as one unit.
2. **Every adjacent pair sheds its `>`.** `</span` sheds onto `<b`, `</b` sheds onto `<a` — a
   mid-run element both receives its predecessor's `>` and sheds its own. No opening tag is
   stranded at a line end after a space anywhere in the chain.

A pair whose second element is *not* the flat hug-both layout (it block-styles, or is a
special element) keeps an intact `>` at that boundary; nothing is ever stranded. The `>`
moves only inside an end tag, so the output parses to a byte-identical AST — render-safe
(confirmed by `ast_diff --render`).

tsv: the run on its own line, `>` dangling between each glued pair, the wide `<a>` wrapping
its attributes. Prettier keeps `<span>a</span><b>b</b><a` glued on the text line and dangles
the `<a>` attributes instead — see `output_prettier.svelte`. `unformatted_ours_compact.svelte`
is the compact authoring (tsv → `input.svelte`, prettier → a dangled form, never `input`).

## Reason

Design choice. The "glued runs travel together" half of the break-before posture: an opening
tag never sits at a line end after a space, and a glued run is one indivisible unit for the
break-before decision. Render-free under Svelte 5 (whitespace boundary) / tag-internal (each
`>` move). See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
