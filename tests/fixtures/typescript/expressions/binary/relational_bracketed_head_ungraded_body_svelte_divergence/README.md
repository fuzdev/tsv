# Bracketed type-argument head that grades no body — Svelte Divergence

**A known over-rejection, not a sanctioned reading.** Every line of `input.svelte` is a
comparison chain to **tsc and to acorn-typescript alike**, and tsv rejects it.

A `<` opening on a `{` or a `[` commits to a type-argument list on any matching `>` that a
line terminator, a `(` or a template follows — the body is not read at all — so the type
parse then rejects a region whose content is a value. `tsv_rejects.txt` pins tsv's own
error on the first line; the canonical AST in `expected_svelte.json` is the comparison
chain acorn-typescript builds, and tsc reads the same chain.

The reason the body is not read is the **printer**, not the grammar. A refusal would have
to hold on tsv's own output as well as on the authored bytes, and neither delimiter gives
it that: neither is a shell the printer strips, so the relaxed reading cannot look through
one the way it does through a `(`; neither opens a region the printer's kept-shell rule
puts a paren pair around; and the printer parenthesizes freely INSIDE both bodies — a
spread argument, an `as` left operand, a for-init `in` — which moves a refused token one
level deeper than the grade looks. A refusal here would therefore print a bare chain that
tsv's own parse claims back, which is the soundness property `TypeArgScan` states.

The `(` head has that shell and is graded, which is the accepting side —
[relational_paren_head_value_body](../relational_paren_head_value_body/). The bodies tsc
itself rejects are
[relational_bracketed_head_recovered_list](../relational_bracketed_head_recovered_list_svelte_divergence/)
and
[relational_paren_head_param_list](../relational_paren_head_param_list_svelte_divergence/);
those are divergences from acorn alone, where this one is against both oracles.

The **printer** side is unaffected: the chain still takes a paren pair around the `>`'s
left operand at every `{` or `[` head, so tsv never emits one of these bare. See
[conformance_prettier_ts.md §Relational chain type-argument parens](../../../../../../docs/conformance_prettier_ts.md).

Because the canonical parser accepts the input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject), and with no accepted
parse there is nothing for a formatter to claim — hence no `expected.json` and no
format-claim siblings.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections (bracketed type-argument head that grades no body).
