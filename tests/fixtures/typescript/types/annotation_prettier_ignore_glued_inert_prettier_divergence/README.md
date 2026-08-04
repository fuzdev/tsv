# annotation_prettier_ignore_glued_inert_prettier_divergence

A **glued** ignore directive after an annotation's `:` is **inert**: tsv honors
a directive only when it is alone on its line, so the annotated type formats
normally — variable, property-signature, and return-type hosts, union children,
and a multi-line value in a parameter list (which breaks the list by ordinary
layout) alike.

Prettier honors the glued placement, freezing the type (the whole union for a
union child) and gluing a parameter list flat around a multi-line frozen slice.
`prettier_variant_frozen.svelte` holds those prettier-stable forms; tsv
normalizes them to `input.svelte`.

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
