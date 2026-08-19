# Cast's sequence operand, line comment in the leading gap

A `//` between a sequence operand's required pair's `(` and the first operand,
under an `as`/`satisfies` cast (`(// c⏎x, y) as A`). The sequence's own required
parens ARE the pair the printer emits, so the leading run is the pair's — same
gap, same rule as the family's other required pairs.

- **tsv**: keeps the comment inside the parens and takes the expanded shell —
  the comment on the `(` line (glued) or its own line (own-line authoring), the
  operands one indent in, the `)` back out — the rendering every required pair
  in the family takes (the
  [non-null grouped operand](../../non_null/grouped_operand_leading_line_comment_prettier_divergence/),
  the [assignment target](../../assignment/cast_target_leading_line_comment_prettier_divergence/)).

```
const a = ( // c
	x, y
) as A;
```

- **prettier**: hoists the run out of the pair, hanging it in the enclosing
  value gap (`const a = // c⏎(x, y) as A;`) — re-binding it from the operand to
  the whole binding — and is non-idempotent on its own answer: a second pass
  adds the continuation indent (`audit_signature.txt` pins the chain).

A doubled shell (`((// c⏎x, y)) as A`) collapses into the one emitted pair with
the run kept — `unformatted_ours_double_paren`.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
