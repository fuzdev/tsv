# trailing_member_expand_args_long_prettier_divergence

A call with an object argument and a trailing member access (`X.f({…}).member`) that exceeds
print width: the call's arguments fit inline, but appending the trailing member overflows.

tsv: expands the object argument, keeping `.success` on the closing brace (`}).success`)
Prettier: keeps the object inline and breaks the `.success` member onto its own line

The `fits` case (98 chars) stays inline in both. In the `breaks` case the args line is 99 chars,
so appending `.success` (+8) would overflow — a break is forced either way; the two differ only in
_where_ they break.

Because Prettier preserves a multiline object, it keeps tsv's expanded form stable too — so the two
only diverge from compact (or Prettier-authored) source. The Prettier form is therefore pinned as
`prettier_variant_chain_break.svelte` (Prettier keeps it stable; tsv normalizes it back to `input`).

## Reason

Print width, and consistency. tsv wraps a call's arguments the same way regardless of a trailing
member — it expands the arguments rather than special-casing the chain — the same stance as its
handling of module-path calls and single-specifier imports.

See [conformance_prettier.md §TypeScript](../../../../../../../docs/conformance_prettier.md#typescript) (Trailing member after a call with an object argument) and [§Print Width Philosophy](../../../../../../../docs/conformance_prettier.md#print-width-philosophy).
