# union_intersection_parens_leading_line_comment_prettier_divergence

A **leading** line comment on a broken union member
(`type A =\n\t| // leading\n\ta\n\t| b;`), where the comment forces the member
onto its own line.

**tsv**: keeps the member flush under the `|`, using whole-tab indentation only:

```
type A =
	| // leading
	a
	| b;
```

**Prettier 3.9**: renders the union member's 2-column `align(2)` offset as
`tabs + 2 spaces` under `--use-tabs`, so the member sits two columns past the
`|`:

```
type A =
	| // leading
	  a
	| b;
```

Both forms are stable under their respective formatters.

## Reason

Per the Tabs-Only Indentation Philosophy, tsv never mixes tabs with alignment
spaces — when a leading line comment forces a union member onto its own line, tsv
keeps it at the `|`'s own indent level rather than emitting prettier's `align(2)`
sub-tab offset. At `tabWidth = 2` the visual result is equivalent; only the
representation differs. The fixture covers the first member (A), multiple leading
comments (D), and a leading line + trailing block (E); the trailing-comment cases
(B, F, M1, M2) and the conditional `extends` case (G) match both formatters.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Tabs-Only Alignment.
