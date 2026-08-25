# inline_sibling_newline_label_hold_container_prettier_divergence

The sibling-newline flow rule's prose gate, on its holding side, across container kinds: a
one-word run is a label ([inline_sibling_newline_label_hold](../inline_sibling_newline_label_hold_prettier_divergence/))
and holds its authored newlines at the root, in an inline element, in a list item and a table
cell, and inside a control-flow block or a snippet body alike. The flowing side already pins that
the container is not an axis
([container_kind_newline_flow](../container_kind_newline_flow_prettier_divergence/)); this is the
same assertion for the hold, which a rule keyed on the container would fail from the other side.

## Reason

Design choice — one predicate, asked of the run and its neighbours, never of what encloses them.
Every held case is identical in both files and agreement with prettier. The control is the
divergence: a two-word run in the same inline container is prose and flows, which prettier holds
(`prettier_variant_newline.svelte`, normalized to `input.svelte` by tsv).

`variant_space.svelte` is the null control: the rule holds an authored newline and never forces
one, so the space spelling of every label shape stays inline under both formatters in every
container (dual-stable).

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
