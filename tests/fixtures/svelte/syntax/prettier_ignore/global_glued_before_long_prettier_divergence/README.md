# global_glued_before_long_prettier_divergence

A frozen global `svelte:*` element glued to the text before it ends that text's line: the
break after it is not part of the glued unit's width. The unit — the word, the directive and
the frozen element — stays on its line when it ends that line at exactly 100, and travels to a
fresh line whole when it would end it at 101.

- `output_prettier.svelte` — prettier keeps the 101 unit on the prose line.
- `unformatted_ours_space_after.svelte` — `input.svelte` with a space after each element in
  place of the break: tsv normalizes it to `input.svelte`, prettier to
  `prettier_variant_space_after.svelte`.
- `prettier_variant_space_after.svelte` — prettier's form of it: the 100 case keeps the follower
  on the line, past 100, and the 101 case keeps the unit on the prose line; tsv normalizes it to
  `input.svelte`.

All of them render identically.

## Reason

Design choice and convergence, render-free under Svelte 5. The break after the element is the
rule of [global_glued_before](../global_glued_before_prettier_divergence/), which converges the
space and line-break spellings prettier each keeps; the travel is the welded-run
travel every glued unit takes at the line's end
([inline_welded_run_travel_long](../../../elements/inline_welded_run_travel_long_prettier_divergence/)).

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
