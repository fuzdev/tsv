# type_params_prettier_ignore_glued_inert_prettier_divergence

A **glued** ignore directive before a type parameter is **inert**: tsv honors a
directive only when it is alone on its line, so the parameter formats normally —
a multi-line parameter expands the declaration's `<…>` by ordinary layout
(function host), and the always-inline method-signature path keeps the brackets
glued.

Prettier honors the glued placement, freezing the parameter and gluing the
angle brackets flat around the frozen slice. `prettier_variant_frozen.svelte`
holds that prettier-stable form; tsv normalizes it to `input.svelte`.

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
