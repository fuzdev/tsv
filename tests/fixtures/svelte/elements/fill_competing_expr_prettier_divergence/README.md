# fill_competing_expr_prettier_divergence

An inline element whose fill content holds **two** breakable expressions and overflows. The
run resolves at the render-free whitespace boundary in front of the first welded unit that
no longer fits — `text2{…}` travels whole to the next line — so **neither** expression
breaks and every line holds inside 100 columns. Prettier keeps the whole run on one line,
overshooting to 102, and breaks the second expression at its `!==` operator.

Prettier also keeps the traveled form, so `input.svelte` is a fixed point of **both**
formatters and the divergence is one of normalization — which form the other authorings of
the content boundary converge to. The travel rule itself is
`fill_multi_expr_travel_long_prettier_divergence`'s; what this fixture pins is the
**convergence across boundary authorings**:

- `unformatted_ours_hug.svelte` is the same document authored with the content hugging the
  opening tag: tsv → `input.svelte`; prettier reads the hugged boundary as an instruction
  and dangles the opening delimiter instead.
- `prettier_variant_dangle.svelte` is that dangled form (prettier-stable, overshooting at
  102 with the second ternary opened mid-line); tsv likewise normalizes it to
  `input.svelte` — see
  [§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

Historically this shape oscillated when the boundary authorings were handled by stranding
expression groups flat under a fits()-Break `line` — each pass left a *different* expression
flat, so two layouts alternated forever. The pairwise whole-flat boundary measurement asks a
position-independent question, so every authoring converges on `input` in one pass.

## Reason

Print width. tsv treats printWidth as a hard limit; the whitespace boundary in front of the
overflowing unit is render-free, so it spends that break — keeping both expressions whole —
where prettier overshoots and tears one open.

See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
