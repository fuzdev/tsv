# inline_component_wide_longname_long_prettier_divergence

The length-independence counterpart of `inline_component_wide_long`: the inline
component here has a long tag name (`<VeryLongComponentName>`). tsv still drops the
whole component to its own line and keeps the preceding word (`and`) hugged on the
text line — the boundary break does not depend on how long the tag name is.

tsv: word stays hugged, the whole component moves to its own line (each line ≤100)
Prettier: keeps `and <VeryLongComponentName` on the text line (101) and breaks the
component internally — see `prettier_variant_attrs_hug.svelte` (prettier's stable
form, which tsv normalizes back to `input.svelte`).

This case exists because a naive fix that relies on a break point *inside* the
opening tag only works for short tag names; the break must sit at the boundary
before the component, so it must hold for arbitrarily long names too.

## Reason

tsv treats printWidth as a hard limit and keeps the component intact rather than
splitting its attributes/closing `>`, so an over-wide component goes to its own
line. The boundary before the component is a collapsible space, so the word before
it stays on the text line. See
[conformance_prettier.md §Inline content hug](../../../../../docs/conformance_prettier.md#svelte-elements).
