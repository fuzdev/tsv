# svelte_element_class_whitespace_prettier_divergence

prettier-plugin-svelte 4.x no longer collapses repeated whitespace in a `class`
attribute on `<svelte:element>` — `class="a   b    c"` is preserved verbatim. On a
**plain** element (`<div class="a   b   c">` → `class="a b c"`) it still collapses,
and so does tsv everywhere, so the divergence is specific to `<svelte:element>`.

tsv: `<svelte:element this="div" class="a b c">` (collapsed, consistent with `<div>`)
Prettier: `<svelte:element this="div" class="a   b    c">` (preserved, only here)

## Reason

**Prettier bug.** The `<svelte:element>` attribute path regressed off the normal
attribute printer in the 4.x modern-ast migration — it no longer runs the
whitespace-collapsing that every other element's `class` goes through (3.5.2
collapsed on `<svelte:element>` too). tsv collapses uniformly across all elements.

`input.svelte` (single-spaced) is stable under both formatters.
`prettier_variant_spaces.svelte` holds the multi-space form: Prettier keeps it
stable (a prettier fixed point), while tsv normalizes it to `input.svelte`.

Sibling of [svelte_element_this_string](../svelte_element_this_string_prettier_divergence/),
the other `<svelte:element>` printer regression from the same migration. Retires
once a plugin fix releases and tsv's oracle is re-pinned.

See [conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
