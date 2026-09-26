# global_glued_before_neighbours_prettier_divergence

A frozen global `svelte:*` element with whitespace after it, whatever stands in front of its
directive: the directive starting its line after a block element, a component glued to the
directive, a space before the directive. In each the boundary after the element is a line
break, as it is after the element unfrozen; a `prettier-ignore-start` / `-end` range around the
element is the control that already takes that break.

Prettier keeps each spelling of that whitespace as its own stable form; tsv converges them on the
break.

- `prettier_variant_space_after.svelte` — every element, and the range, followed by a space:
  prettier keeps it, tsv normalizes it to `input.svelte`.

Both render identically.

## Reason

Design choice and convergence, render-free under Svelte 5 — the rule of
[global_glued_before](../global_glued_before_prettier_divergence/): the compiler hoists the
element out of the fragment, so the break after it renders the one space the authored
whitespace did, and the element takes the break it takes unfrozen.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
