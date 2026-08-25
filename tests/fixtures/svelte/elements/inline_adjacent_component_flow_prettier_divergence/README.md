# inline_adjacent_component_flow_prettier_divergence

The sibling-newline flow rule at the **adjacent** separator when one side of it is a **component**.
[inline_adjacent_sibling_newline_flow](../inline_adjacent_sibling_newline_flow_prettier_divergence/)
pins the whitespace-only separator between two `<span>`s and between two tags; this fixture puts a
component on one or both sides of that same separator, beside an element pair in the same shape.

A component is inline flow content exactly as a `<span>` is — the flow predicate answers them alike —
so when the run holds prose, the separator between two components (or between an element and a
component, in either order) is a spelling of a space and the run packs per width. Every boundary tsv
collapses here is inter-node whitespace that renders as one space either way.

Prettier's `isInlineElement` requires a `RegularElement`, so a component is neither inline nor block
to it: the whitespace-only node before a component is printed as a plain `line` that breaks with the
container, while the text-adjacent boundaries of the same run hug in its fill. From the **space**
spelling it therefore emits `text1 <Comp1 />⏎<Comp2 /> text2` whenever the container is multiline —
one run, two answers, in a line that fits — which `output_prettier.svelte` records; from the
**newline** spelling it holds every line (`prettier_variant_newline.svelte`, which tsv normalizes to
`input.svelte`). The `<span>` pair beside the first case is the parity assertion: both formatters
pack it, so what differs is only the sibling's kind.

Two controls pin the rule's edges, as in the element fixture:

- **structural cause** — the same run in a container made multiline by a block child rather than by
  its own newlines, so the container's cause is not what decides the run.
- **prose-free run** — two components with no text anywhere in the run. This does **not** flow:
  with no fill to reflow into, the authored newlines are the author's only structure. It is
  identical in both files.

## Reason

Design choice: tsv converges the space- and newline-authored spellings of one document onto a single
fixed point and lays a component out as the inline sibling it is, where prettier holds a distinct
form per authoring and splits a component off a line that fits.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
