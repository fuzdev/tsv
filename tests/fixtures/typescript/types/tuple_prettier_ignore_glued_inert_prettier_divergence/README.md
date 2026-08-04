# tuple_prettier_ignore_glued_inert_prettier_divergence

A **glued** ignore directive before a tuple member is **inert**: tsv honors a
directive only when it is alone on its line, so the member and the list format
normally — inline lists stay inline, and a multi-line member keeps its authored
break under ordinary layout, breaking the tuple one member per line.

Prettier honors the glued placement and freezes the member — for a multi-line
member it keeps the container flat and glues the separators around the frozen
slice. `prettier_variant_frozen.svelte` holds those prettier-stable forms; tsv
normalizes them to `input.svelte`. `unformatted_ours_perturbed.svelte` carries
whitespace perturbations only tsv normalizes.

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
