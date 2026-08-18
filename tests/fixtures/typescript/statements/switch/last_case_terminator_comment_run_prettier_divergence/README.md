# last_case_terminator_comment_run_prettier_divergence

A case's last statement with a `//` in its pre-`;` terminator gap and a second
comment — a `//` or a block — trailing the `;`: two comments deferred to one flush.
The `;`-line comment lands **own-line at the case's level** (where a post-consequent
comment settles), emitted from the builder with the clause-tail dedent so the landing
is a one-pass fixed point at the last case too, whose enclosing break is the switch's
`}` one level out (`docs/comments.md` §Trailing and dangling runs; the renderer-side
bound is cataloged in
[deferred_comment_run_separator](../../../syntax/comments/deferred_comment_run_separator_prettier_divergence/)).
A dropped `;;` and a sibling-case bound land identically.

**tsv**: each comment keeps its own line, in authored order:

```
switch (a) {
	case 1:
		fn1(); // c1
	// c2
}
```

**prettier**: from the pre-`;` authoring
(`unformatted_ours_pre_terminator.svelte`) it **welds** a `//` pair
(`fn1(); // c1 // c2` — the second comment becoming text of the first, content
loss) and **reorders** a block trailer ahead of the authored `//`
(`fn5(); /* c8 */ // c7`), landings both formatters then keep
(`variant_pre_terminator.svelte`). `input` is a fixed point for both.

Reason: print-once over the weld and authored order over the reorder — the same
stance as the decorator and `;`→`else` trailer entries. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
