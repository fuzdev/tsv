# params_prettier_ignore_glued_inert_prettier_divergence

A **glued** ignore directive before a parameter is **inert**: tsv honors a
directive only when it is alone on its line, so the parameter and the list
format normally — inline lists stay inline, and a multi-line parameter keeps its
authored break under ordinary layout, breaking the list one parameter per line.
Value-side (`function`) and signature-side (function type) parameter lists
classify identically.

Prettier honors the glued placement and freezes the parameter — for a multi-line
parameter it keeps the list flat and glues the separators around the frozen
slice. `prettier_variant_frozen.svelte` holds those prettier-stable forms; tsv
normalizes them to `input.svelte`.

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
