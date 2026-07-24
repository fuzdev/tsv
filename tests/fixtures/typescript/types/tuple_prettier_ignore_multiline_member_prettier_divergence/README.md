# tuple_prettier_ignore_multiline_member_prettier_divergence

A `/* prettier-ignore */` glued to a tuple member whose frozen slice spans multiple
lines (an object type literal kept verbatim).

**tsv** breaks the tuple one member per line — its layout whenever a member spans
lines — so the frozen slice sits on its own member line with the parent-owned `,`
after it:

```ts
type T = [
	a,
	/* prettier-ignore */ {
		x:   1
	},
	b
];
```

**Prettier** keeps the container flat, gluing the separators around the verbatim
slice (`output_prettier.svelte`) — its printed-ignored slice is a plain string doc,
invisible to its break propagation:

```ts
type T = [a, /* prettier-ignore */ {
		x:   1
	}, b];
```

`unformatted_ours_flat.svelte` is that flat form: tsv normalizes it to
`input.svelte` (the must-break fires on the multi-line slice); prettier keeps it
flat.

## Reason

Holding the tuple's per-line layout when a frozen member is multi-line keeps the
frozen slice visually delimited and the members uniformly presented, rather than
gluing reformatted members onto the lines of an opaque verbatim block. A
single-line frozen member keeps the width-decided layout (matching prettier — see
the ordinary `tuple_prettier_ignore_glued_member` fixture). The tuple analog of
`union_prettier_ignore_multiline_member_prettier_divergence`.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
