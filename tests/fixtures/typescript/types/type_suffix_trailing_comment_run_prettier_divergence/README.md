# Divergence: type-suffix trailing comment **run** stays inside the brackets

The multi-comment face of
[type_suffix_trailing_comment](../../declarations/variable/type_suffix_trailing_comment_prettier_divergence/),
in type-alias position: a run of line comments at the end of an indexed access's brackets
(`T[K // c1⏎// c2⏎]`) or of a retained paren shell (`(U // c4⏎// c5⏎)[]`). tsv keeps the whole
run inside the region, distinct, one comment per line. **Prettier carries the run out past the
`;`**, where the first comment trails the statement and the rest read as leading whatever
follows.

```ts
// tsv (run stays inside)   // prettier (carried out past the `;`)
type A1 = T[                type A1 = T[K]; // c1
	K // c1                  // c2
	// c2
];
```

## Reason

Same rule as the single-comment fixture — the four sibling bracketed type regions (`{}`, `<>`,
tuple `[]`, function-type `()`) already keep a trailing comment inside in **both** formatters,
so the indexed access and the stripped paren shell are prettier's two exceptions and tsv's
convergence targets.

The run is where the cost of carrying comments out shows up directly: once a run is on a line
it shares with another construct's escaped comment, the two render back to back and the second
`//` becomes text of the first. Keeping the run inside the brackets means the collision cannot
be assembled in the first place.

`unformatted_ours_flat.svelte` carries the flat authoring, which reaches `input` under tsv
only.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
