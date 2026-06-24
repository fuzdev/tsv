# Sequence operand edge comment divergence (statement context)

Like [operand_edge_comment](../operand_edge_comment_prettier_divergence/), but
the sequence is the whole right-hand side of a statement, so its trailing edge
sits right before the terminating `;`. The float behaves the same — a leading
comment on the first operand floats before the opening `(`, a trailing comment
on the last operand floats after the closing `)` — but the trailing one then
lands *before* the `;`:

- `const a = ((/* c */ x), y);` → `const a = /* c */ (x, y);`
- `const b = (x, (y /* c */));` → `const b = (x, y) /* c */;`

For the **leading** case tsv reaches prettier's fixed point (prettier just takes
two passes to get there, so `unformatted_ours_paren` is paired only with our
formatter, not prettier's first pass).

The **trailing** case is the divergence: tsv keeps the comment before the `;`
(`(x, y) /* c */;`), preserving its association with the operand, while prettier
floats it *past* the `;` (`(x, y); /* c */`) — see `output_prettier.svelte`.
Keeping a trailing comment before the terminating `;` is consistent with tsv's
broader before-semicolon comment handling. In the call-expression context there
is no `;` at the edge, so there both formatters agree on `(x, y) /* c */`.

Reason: comment relocation — the trailing comment stays before `;` (prettier
floats it past). See
[conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
