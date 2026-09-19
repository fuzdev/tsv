# curried_callee_pair_comment_prettier_divergence

A block comment inside the paren pair around a curried arrow chain that is a call's callee
(`(/* c */ (a) => (b) => …)()`, `((a) => (b) => … /* t */)()`).

**tsv**: the pair stays closed against the chain and holds the comment where it was
written — the leading block leads the first head, the trailing one trails the body:

```ts
(/* c */ (aaaa) =>
	(bbbb) =>
	(cccc) =>
		dddd)();
```

**Prettier**: opens the pair onto its own lines and mangles the interior — a fabricated
blank line above the `)` for the leading comment, and a trailing comment on a line of its
own at a three-column indent no other construct produces:

```ts
(
	/* c */ (aaaa) =>
		(bbbb) =>
		(cccc) =>
			dddd

)();
```

Prettier is also non-idempotent on its own answer here (`audit_signature.txt` pins the chain).

## Reason

With no comment, the callee's pair opens onto its own lines with the heads at one shared
indent — prettier's `isCallee` chain shape, which tsv matches (the null control, and
[curried_callee](../curried_callee/)). That shape opens the pair from INSIDE the chain's
own doc, while a commented pair's two gaps are printed by the pair's shell builders; the
two cannot both own those lines, so a commented pair keeps the shell form. Prettier's
output there is no guide — neither of its forms is one it emits anywhere else — so the
position is preserved as written, per
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
