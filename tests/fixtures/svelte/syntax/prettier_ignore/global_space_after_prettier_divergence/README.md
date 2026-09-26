# global_space_after_prettier_divergence

The boundary after a frozen global `svelte:*` element is the printer's wherever the directive
sits and whatever the element spans: a directive on its own line with prose after the element, a
directive glued to another global element (itself hoisted, so it takes its own line in front),
and a frozen `<svelte:head>` that spans lines. The whitespace after the element is a line break,
as it is after the element unfrozen.

Prettier keeps each spelling of that whitespace as its own stable form; tsv converges them on the
break.

- `prettier_variant_space_after.svelte` — every element followed by a space, and the global
  element in front of the second directive glued to it: prettier keeps both, tsv normalizes them
  to `input.svelte`.

Both render identically.

## Reason

Design choice and convergence, render-free under Svelte 5 — the rule of
[global_glued_before](../global_glued_before_prettier_divergence/): the compiler hoists the
element out of the fragment, so the break after it renders the one space the authored
whitespace did. The break is owed after the element's last byte, so a frozen element spanning
lines takes it after its closing tag.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
