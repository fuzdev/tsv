# pre_special_element_boundary_prettier_divergence

A `<svelte:boundary>` inside `<pre>` lays out like every other element of the
whitespace-sensitive family (`<Comp>`, an inline element, `<svelte:element>`): content holding a
line break moves the `>` to the next line and dangles the closing tag's `>`, a line past 100
columns dangles the closing `>`, and a wrapping attribute list does the same. At exactly 100
columns nothing moves.

- `input.svelte` — tsv's form.
- `unformatted_ours_compact.svelte` — every tag whole; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier never breaks before a boundary's `>` or dangles its closing
  `>`: the line-broken content keeps both tags whole, the 101-column line runs over, and the
  wrapped attribute list ends on a whole closing tag.

Every break lands inside a tag, so both forms render the same `<pre>` text.

## Reason

Design choice: prettier-plugin-svelte exempts `SvelteBoundary` from hugging for every position,
which inside `<pre>` leaves its `>` and its closing tag no break point. Nothing about the render sets a boundary
apart there, so tsv keeps the family's one rule. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
