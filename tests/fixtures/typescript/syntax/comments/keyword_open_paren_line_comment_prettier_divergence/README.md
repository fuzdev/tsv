# keyword_open_paren_line_comment_prettier_divergence

A `//` line comment the author placed on the grouping `(` line of a `return` / `throw`
argument stays trailing the `(`; prettier moves it to its own line.

tsv:

```ts
return ( // c
	a = b
);
```

Prettier relocates `// c` onto its own line:

```ts
return (
	// c
	a = b
);
```

A leading comment before the argument forces the grouping parens to break either way (the
argument is a restricted-production operand, so the break is legal only inside parens) —
the only question is whether the comment leads the argument line or trails the `(`. tsv
preserves where the author wrote it; prettier always leads. The rule is independent of the
argument kind: an assignment (`a = b`), a sequence (`a, b`), and a plain expression (`a`)
all behave alike, in both `return` and `throw`.

Only the **same-line `//`** authoring diverges. A comment the author put on its **own
line** below the `(` (`return (⏎// c⏎a`) keeps that line in both formatters (the sibling
[value_leading_comment_parens](../../../expressions/sequence/value_leading_comment_parens/)
covers that non-divergent shape), and a same-line **block** comment is not covered here.

`yield` / `yield*` are the third restricted production and share the hanging-paren layout,
but prettier prints their comment differently (`yield // c⏎(a, b)`, not the clean own-line
relocation), so they are handled separately.

## Reason

Comment placement is a deliberate authoring choice and tsv preserves it. This is the
`return`/`throw` open-paren analog of the type-alias `=`
[rhs_line_comment_same_line](../../../types/aliases/rhs_line_comment_same_line_prettier_divergence/)
and the attribute-list
[comment_same_line](../../../../svelte/attributes/comment_same_line_prettier_divergence/)
divergences — a same-line `//` stays trailing its token. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [value_leading_comment_parens](../../../expressions/sequence/value_leading_comment_parens/) — the own-line authoring (matches prettier, no divergence)
- [rhs_line_comment_same_line](../../../types/aliases/rhs_line_comment_same_line_prettier_divergence/) — the type-alias `=` same-line `//` analog
- [comment_same_line](../../../../svelte/attributes/comment_same_line_prettier_divergence/) — the attribute-list same-line `//` analog
