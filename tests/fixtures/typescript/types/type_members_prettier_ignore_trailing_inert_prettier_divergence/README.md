# type_members_prettier_ignore_trailing_inert_prettier_divergence

A directive **trailing a member's line** is **inert** at type-literal and
interface member positions: tsv honors a directive only when it is alone on its
line, so nothing freezes in either direction and the members format normally.

Prettier honors the placement backward — the trailing directive freezes the
**preceding** member. `prettier_variant_frozen.svelte` holds that
prettier-stable form (`a:   1;` kept frozen); tsv normalizes it to
`input.svelte`.

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
