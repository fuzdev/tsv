# clauses_prettier_ignore_glued_inert_prettier_divergence

A directive that shares its line with anything else — **glued** to the value, or **trailing
an opening delimiter** — in a value head's gap — a `for` header clause, a `return`
operand, a condition head, an assignment operator, an object property value, a unary
operator, a template `${`, a computed key's `[` — is **inert**: tsv honors a directive only
when it is alone on its line, so the value formats normally.

Prettier honors the glued placement and freezes the value.
`prettier_variant_frozen.svelte` holds those prettier-stable forms; tsv normalizes them to
`input.svelte`.

The trailing-a-delimiter cases (a computed key's `[`, a spread's `...`) also assert that tsv
**keeps** the directive on that line rather than forcing it onto its own — moving it would
silently arm a freeze the author's placement never asked for. Prettier writes the same
placement there and freezes anyway. The unary comment-holder `(` makes the same claim in its
own fixture (prettier relocates at that one delimiter, so it needs an `output_prettier`):
[unary/paren_glued_line_comment](../../../expressions/unary/paren_glued_line_comment_prettier_divergence/).

The own-line placement, which both tools honor, is the ordinary sibling
[clauses_prettier_ignore_head](../clauses_prettier_ignore_head/) (and
[return_throw/operand_prettier_ignore_head](../../return_throw/operand_prettier_ignore_head/)).
For the three value heads added last — the unary operand, the template interpolation and the
computed key — those siblings are
[unary/operand_prettier_ignore_head](../../../expressions/unary/operand_prettier_ignore_head/),
[template/interpolation_prettier_ignore_head](../../../expressions/literals/template/interpolation_prettier_ignore_head/)
and
[objects/computed_key_prettier_ignore_head](../../../expressions/objects/computed_key_prettier_ignore_head_prettier_divergence/).
The spread's `...`→argument gap pins its own glued case, beside its head claim, in
[spread/argument_prettier_ignore_head](../../../expressions/spread/argument_prettier_ignore_head_prettier_divergence/).

## Reason

The placement rule is total and exception-free: a directive freezes the following
construct only when it is alone on its line. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
