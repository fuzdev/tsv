# union_redundant_paren_member_mixed_trailing_line_comment_prettier_divergence

The mixed / trailing extension of
[union_redundant_paren_member_line_comment](../union_redundant_paren_member_line_comment_prettier_divergence/):
a redundant paren shell around a **later** union member holds a leading **line**
comment together with a **leading block** (mixed, `A | (/* b */ // c\n B)`) or a
**trailing block** after the member (trailing, `A | (// c\n B /* t */)`), and the
double-nested forms — plus the run of **blocks alone** (`A | (/* b2 */\n/* c2 */\n B)`),
where no `//` is involved and what takes the run out of the shell is a block the author
ISOLATED on its own line.

**tsv** strips the parens (they don't survive, so the run cannot stay "inside")
and renders it between the `| ` members — above the member's `| `, a trailing
block staying inline after the member, each comment kept where the author wrote
it:

```ts
type U1 =
	| A
	/* b */ // c
	| B;

type U2 =
	| A
	// c
	| B /* t */;

type U3 =
	| A
	/* b2 */
	/* c2 */
	| B;
```

The run keeps the author's own **glue**: `/* b */` shares the line comment's line
because that is where it was written. `U3` is the same rule with no `//` at all — an
own-line block is authoring signal exactly as a `//` is, and a shell that strips has no
line of its own to keep it on, so the member gap does. Left inside the shell it came back
glued after the `| `, which the reparse reads as an inline block: nothing forced the
member break any more and the union printed flat on the next pass. Only the run's POSITION is at issue below —
its interior is prettier's leading-comment rule, which both formatters apply the
same way here.

**Prettier** floats the leading run across the member boundary to **trail the
previous member** (`| A /* b */ // c`, `| A /* b2 */`), keeping the rest inline —
`variant_trailing.svelte`. Both forms are dual-stable (each formatter keeps its
own), so this is a `variant_*` divergence, exactly as the pure-line sibling.

The `unformatted_ours_*` variants are the paren shells; tsv normalizes them to
`input` in one pass, prettier floats them to `variant_trailing` instead (N6/N10).
Per Comment Position Philosophy, tsv associates the run with the member it
documents (`B`) rather than hoisting it onto the previous member (`A`).

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
