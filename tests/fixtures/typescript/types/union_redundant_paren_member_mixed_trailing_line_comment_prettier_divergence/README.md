# union_redundant_paren_member_mixed_trailing_line_comment_prettier_divergence

The mixed / trailing extension of
[union_redundant_paren_member_line_comment](../union_redundant_paren_member_line_comment_prettier_divergence/):
a redundant paren shell around a **later** union member holds a leading **line**
comment together with a **leading block** (mixed, `A | (/* b */ // c\n B)`) or a
**trailing block** after the member (trailing, `A | (// c\n B /* t */)`), and the
double-nested forms.

**tsv** strips the parens (they don't survive, so the run cannot stay "inside")
and renders it between the `| ` members — the block and line each on their own
line before the member's `| `, a trailing block staying inline after the member,
each comment kept where the author wrote it:

```ts
type U1 =
	| A
	/* b */
	// c
	| B;

type U2 =
	| A
	// c
	| B /* t */;
```

**Prettier** floats the leading run across the member boundary to **trail the
previous member** (`| A /* b */ // c`), keeping the rest inline —
`variant_trailing.svelte`. Both forms are dual-stable (each formatter keeps its
own), so this is a `variant_*` divergence, exactly as the pure-line sibling.

The `unformatted_ours_*` variants are the paren shells; tsv normalizes them to
`input` in one pass, prettier floats them to `variant_trailing` instead (N6/N10).
Per Comment Position Philosophy, tsv associates the run with the member it
documents (`B`) rather than hoisting it onto the previous member (`A`).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
