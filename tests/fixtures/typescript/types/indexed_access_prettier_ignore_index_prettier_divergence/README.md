# indexed_access_prettier_ignore_index_prettier_divergence

An own-line directive between an indexed-access type's `[` and its index type
freezes the index — and tsv keeps the directive **own-line** inside the brackets,
where the author put it:

```ts
type A = { b: 2 }[
	// prettier-ignore
	{x:   1}
];
```

Prettier instead glues the directive to trail the `[` (`[// prettier-ignore`),
freezing the index on its first pass (`output_prettier.svelte`) — but that form is
not self-stable: prettier's own second pass collapses the indexed access inline and
floats the comment to trail the whole statement
(`type A = { b: 2 }[{ x: 1 }]; // prettier-ignore` — freeze lost, non-idempotent,
pinned via `audit_signature.txt`). A directive sharing the `[`'s line is inert under
tsv's placement classification, so the authored own-line placement is the only form
that holds the freeze.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
