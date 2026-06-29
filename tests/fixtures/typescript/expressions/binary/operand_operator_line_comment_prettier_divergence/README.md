# operand_operator_line_comment_prettier_divergence

A line comment after a binary chain's **left operand**, before the operator
(`1 // c\n+ 2`). Covers an arithmetic operator (`+`), a logical operator (`&&`),
and the keyword-binary operators (`in`, `instanceof`) — all `BinaryExpression`
through the one binary-chain printer.

**tsv**: keeps the comment trailing the operand where the author wrote it and
drops the operator, glued to its right operand, to the next line:

```
const a =
	1 // c
	+ 2;
```

**Prettier**: relocates the comment *past* the operator, keeping the operator
trailing the left operand:

```
const a =
	1 + // c
	2;
```

## Reason

The comment sits in the operand→operator gap and trails the left operand on its
source line — a position that carries authorship signal (it comments the left
operand, not the operator). tsv preserves it there ([Comment Position
Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
Principles 1 and 2); the `//` then forces the operator to the next line, where it
hugs its right operand rather than over-breaking onto a line of its own
(`1 // c\n+\n2`). Prettier instead moves the comment across the operator, changing
its apparent association to the operator/right operand.

A **same-line block comment** (`1 /* c */ + 2`) stays inline in both formatters
and is not a divergence. Both forms are content-preserving and idempotent in their
respective formatters.

The two comment positions are **dual-stable in tsv**: prettier's relocated output
(comment after the operator) is itself a different authored position — a trailing
comment in the operator→operand gap — which tsv also keeps where it is,
idempotently (the `variant_comment_after_operator.svelte` form pins this; tsv does
not "fix" prettier's relocation back to input, which would itself be a
position-changing relocation). The fixture system formats `output_prettier.svelte`
only to check it equals `prettier(input)`, never through tsv, so the `variant_*`
is what asserts tsv preserves that position too.

Previously tsv emitted the comment inline and **swallowed the operator and right
operand** (`const a = 1 // c + 2;` — `+ 2` absorbed into the comment, a
non-idempotent content loss); keeping the comment trailing the operand with the
operator on the next line fixes the loss.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation
("Binary operand to operator line comment").
