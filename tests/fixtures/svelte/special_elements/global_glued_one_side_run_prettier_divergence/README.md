# global_glued_one_side_run_prettier_divergence

Global `svelte:*` elements glued to text on **one** side only, in the two shapes
[global_glued_one_side](../global_glued_one_side_prettier_divergence/) does not reach: a run of two
elements with whitespace **inside** the run (each end of the run glued to its text), and a
`<svelte:head>` with content. tsv gives each element its own line from every authoring.

- `unformatted_ours_spaced_run.svelte` — the run spaced inside, the head glued after: tsv
  normalizes it to `input.svelte`, prettier to `prettier_variant_spaced_run.svelte`, which breaks
  the run at its space and keeps each element on its glued text's line.
- `prettier_variant_head_glued_before.svelte` — the head glued before, spaced after: prettier
  keeps it, tsv normalizes it to `input.svelte`.

All of them render identically.

## Reason

Design choice, render-free under Svelte 5. The compiler hoists these elements out of the fragment
before its whitespace rules run, so a break on the glued side merges with the whitespace on the
other side into the one rendered space that was already there, and tsv spends it on the element's
own line. Whitespace inside the run is the same case: that whitespace is each element's direct
neighbour on its inner side, so neither element is glued there. Glued on **both** sides the break
is not free, and the element stays on the content's line under both formatters
([global_glued_both_sides_neighbour_kinds](../global_glued_both_sides_neighbour_kinds/) holds the
run glued at both ends).

See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
