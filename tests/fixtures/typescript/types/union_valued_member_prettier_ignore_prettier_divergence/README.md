# union_valued_member_prettier_ignore_prettier_divergence

An own-line directive above a **union-valued** list member — a tuple element or a
sole type argument whose value is a union — freezes the **whole member** in tsv
(Rule A: the container's gap freezes its member, whatever the member's shape,
operators and all):

```ts
type A = [
	// prettier-ignore
	{a:   1} | {b:   2},
	C
];
```

Prettier's union `types[0]` redirect reaches *into* the member and freezes only the
union's **first member**, reformatting the rest (`{a:   1} | { b: 2 }`,
`output_prettier.svelte`, self-stable). The item-level scope is the consistent
reading of the list rule — the directive targets the member the author pointed at,
not a fragment of it. Both frozen slices carry perturbations in both union arms so
either scope reading is visible.

Hosts covered: tuple element and sole type argument.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
