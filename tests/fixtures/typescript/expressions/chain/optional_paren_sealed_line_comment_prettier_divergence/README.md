# Sealed optional-chain pair, line comment in the trailing gap

The `//` spelling of
[optional_paren_sealed_comment](../optional_paren_sealed_comment/): a comment
between a sealed parenthesized optional chain and its `)`, where the pair is
required *without* a `!` (`(a?.b).ccc` seals the chain on its own). The `!`
sibling is
[optional_paren_non_null_sealed_line_comment](../optional_paren_non_null_sealed_line_comment_prettier_divergence/),
and both spellings answer the gap the same way.

- **tsv**: the comment stays inside the parens, where the author wrote it. A `//`
  cannot trail inline before the `)` — it would swallow it — so the pair takes its
  expanded shell (chain one indent in, `)` back out), the same layout the leading
  gap already takes.
- **prettier**: relocates the comment past the `)` and breaks the chain around it,
  stranding the call's `()` / the template on the next line.

```
const m = (             const m =
	a?.b // c1              (a
).ccc;                        ?.b) // c1
                          .ccc;
```

Four positions print the pair and all four take the same answer: a member access,
a call, a computed lookup and a template tag.

With **both** gaps commented (c7–c10) the pair takes ONE expanded shell — the
leading run above the chain, the trailing run beside it, the `)` back out — the
same shape either gap takes alone, and the same one the
[IIFE pair](../../calls/iife_callee_leading_line_comment_prettier_divergence/)
takes, where prettier agrees byte-for-byte.

**The `)` is the boundary.** c5 and c6 are the contrast — a comment the author
wrote *after* the `)` is outside the pair and stays outside, so the two authorings
keep two fixed points under tsv. Prettier collapses them: it pulls the outside
block back in (`(a?.b /* c5 */).ccc`) while pushing the inside `//` out, so its
c1 and c6 outputs are the same file. That is the information the position carries,
and losing it is the reason tsv holds both sides.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
