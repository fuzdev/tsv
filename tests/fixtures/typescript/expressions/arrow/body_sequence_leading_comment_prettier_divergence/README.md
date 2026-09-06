# body_sequence_leading_comment_prettier_divergence

An arrow whose sequence body's **first operand** carries an own-line comment inside its
grouping parens (`() => ((⏎// c⏎a), b)`). The sequence floats that comment out ahead of
its own `(` — tsv's standing form for a sequence's leading edge, in statement, value and
`return` position alike — so the body's doc opens with the comment and a hardline, and a
body that opens that way does not hug the `=>`: it takes the own-line-comment break, run
and sequence at the body's indent (`Printer::arrow_body_hugs`).

tsv (`input.svelte`):

```ts
const f1 = () =>
	// c
	(a, b);
```

Prettier settles on the same form, in two passes: its first keeps the hug and the
sequence's parens, with the comment inside them and the operands broken by the hardline it
carries (`prettier_intermediate_inner_shell.svelte`), and its second — the comment now ahead
of the whole body — breaks below the `=>` exactly as tsv does in one:

```ts
const f1 = () => (
	// c
	a,
	b
);
```

Hugging the floated form instead printed `=> // c⏎(a, b)` flush, which the reparse read
as the `=>`→body gap's own comment and indented — a second fixed point one pass away. Not a
placement difference — the fixed point is shared. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment).
