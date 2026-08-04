# empty_slot_prettier_ignore_inert_prettier_divergence

A directive written in an **empty clause slot** of a `for` header stays in that slot, and so
freezes nothing — the clause it would freeze is on the other side of the `;`:

```ts
for (
	// prettier-ignore
	;
	b < 10;
	c++
) {
	fn();
}
```

Prettier moves it across the `;` into the test clause, where it freezes.
`variant_relocated.svelte` is that relocated authoring — **dual-stable**: prettier keeps it,
and so does tsv, because there the directive really does lead the test clause and freezes it.
The divergence is only about which slot the directive is *moved to*, never about what a
directive in a given slot does.

This is the freeze consequence of the slot rule the sibling
[empty_slot_comment](../empty_slot_comment_prettier_divergence/) already sanctions for ordinary
comments: the slot a comment sits in is what it is about, and relocating across a `;` changes
that association. It follows directly — the directive that freezes a clause is the one printed
above it — so the two rules stay one rule.

## Reason

A directive's placement is the thing that decides what it freezes, so an emitter must never
relocate one. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
