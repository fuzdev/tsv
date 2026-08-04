# type_arguments_prettier_ignore_glued_inert_prettier_divergence

A **glued** ignore directive before a type argument is **inert**: tsv honors a
directive only when it is alone on its line, so the argument formats normally
and inline lists stay inline — in type position, at call sites, and on a sole
argument alike.

Prettier honors the glued placement and freezes the argument.
`prettier_variant_frozen.svelte` holds those prettier-stable frozen forms; tsv
normalizes them to `input.svelte`.

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
