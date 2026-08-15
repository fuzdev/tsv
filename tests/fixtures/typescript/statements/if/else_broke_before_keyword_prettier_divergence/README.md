# else_broke_before_keyword_prettier_divergence

A block comment trailing the `}` of a construct, with the **continuation keyword**
dropped to the next line by the author (`} /* b1 */⏎else {`).

tsv pulls the keyword back up onto the comment's line at every keyword in this gap —
`else`, `catch`, `finally`, and a do-while's `while`. The comment's own position is
unchanged either way; only the keyword's line differs.

| keyword | prettier | tsv |
| --- | --- | --- |
| **`else`** | **keeps the author's break** | pulls the keyword up |
| do-while `while` | pulls the keyword up | pulls the keyword up |
| `catch` / `finally` | relocates the comment into the body — no oracle | pulls the keyword up |

`prettier_variant_broke_before_else.svelte` pins prettier's stable form for the `else`
row; `unformatted_broke_before_while.svelte` pins that the do-while row normalizes to
`input` under **both** formatters.

## Reason

Prettier is split across the one gap, so there is no oracle to follow: matching it at
`else` would mean diverging at `while`, and the reverse. tsv answers the gap once —
the keyword continues the construct, so it hugs whatever trails the `}` — and that
uniformity is what the rule buys. Nothing is lost either way: the comment stays where
the author wrote it, both forms are stable in their own formatter, and prettier's form
is not re-broken by tsv on a second pass (it normalizes to `input` and stays there).

The `catch`/`finally` rows have no oracle at all — prettier moves the comment into the
following block's body, discarding the question — which is the same relocation §"The
`}`→continuation-keyword gap keeps the blank" records for the blank.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation ("Between `}` and `else`").
