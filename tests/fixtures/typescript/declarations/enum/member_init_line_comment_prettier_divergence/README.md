# member_init_line_comment_prettier_divergence

A line comment trailing an enum member's `=` (`A = // c⏎a`).

**tsv** keeps the comment on the `=` line and hangs the value below it, the answer a
declarator initializer and a class field give in both formatters:

```
A = // c
	a,
```

**Prettier** relocates it at the enum member only. It trails the whole member
(`A = a, // c`), or for an assignment value it goes inside the clarity parens after the
inner `=` (`B = (a = // c⏎b)`).

With a second line comment trailing the member (`E2`), prettier's relocation merges the
two onto one line: `A = a, // c1 // c2`. The second `//` is now text inside the first, a
form prettier keeps from then on. tsv keeps both comments distinct.

Prettier's output is also stable in tsv, since tsv keeps a comment wherever the author
put it. `variant_relocated.svelte` pins that. `unformatted_ours_compact.svelte` spells the
members on one line and normalizes to input in tsv only.

## Reason

**◆content_preservation** for the two-comment case, **comment position** for the rest. The
`=`→value gap is one seam at every initializer, and the enum member is the one host where
prettier moves a same-line `//` off it. tsv answers all of them the same way. It is the
same-line twin of the own-line entry
([member_init_prettier_ignore_head](../member_init_prettier_ignore_head_prettier_divergence/)).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
