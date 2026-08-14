# Sealed optional-chain non-null, own-line comments before the `!`

The own-line spelling of
[optional_paren_non_null_sealed_line_comment](../optional_paren_non_null_sealed_line_comment_prettier_divergence/),
at the same gap: comments that hold their own line between the operand and the
`)!` — a lone own-line `//`, and the block half of a mixed run `// c⏎/* d */`
(a block following a `//` is always own-line, since the `//` owns the rest of
its line). Covers the three sealed positions: a `new` callee, a template tag,
and a member access (which reaches the same gap through the chain printer).

- **tsv**: keeps each comment inside the parens on its own line below the
  operand, in authored order — an own-line comment in this gap trails the
  operand, it never leads it.
- **prettier**: relocates the run outside, after `)!`, stranding the call
  arguments, the template, or the member access on the next line — and at the
  member position its fixed point hoists the block *above* the operand,
  reversing the authored order (`/* d */` printed before `(a?.b)! // c`).

Reason: comment preservation — order is position. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Sealed optional chain / non-null operand, own-line
comments) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
