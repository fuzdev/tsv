# clauses_prettier_ignore_glued_inert_prettier_divergence

A **glued** ignore directive in a value head's gap — a `for` header clause, a `return`
operand — is **inert**: tsv honors a directive only when it is alone on its line, so the
clause formats normally.

Prettier honors the glued placement and freezes the value.
`prettier_variant_frozen.svelte` holds those prettier-stable forms; tsv normalizes them to
`input.svelte`.

The own-line placement, which both tools honor, is the ordinary sibling
[clauses_prettier_ignore_head](../clauses_prettier_ignore_head/) (and
[return_throw/operand_prettier_ignore_head](../../return_throw/operand_prettier_ignore_head/)).

## Reason

The placement rule is total and exception-free: a directive freezes the following
construct only when it is alone on its line. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
