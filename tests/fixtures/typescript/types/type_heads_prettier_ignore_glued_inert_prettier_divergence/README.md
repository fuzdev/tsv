# type_heads_prettier_ignore_glued_inert_prettier_divergence

A **glued** ignore directive between a type head and its child is **inert**:
tsv honors a directive only when it is alone on its line, so the child formats
normally — at the alias `=`, a named-tuple `label:`, inside a mapped type's
`[...]` key bracket, and at the mapped `]:` value head.

Prettier honors the glued placement — it freezes the child at the alias and
named-tuple heads, the whole mapped type from the in-bracket key position, and
the value at the `]:` head. `prettier_variant_frozen.svelte` holds those
prettier-stable frozen forms; tsv normalizes them to `input.svelte`.

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
