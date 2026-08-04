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

`B` adds the frozen route's **own** index-to-`]` gap. That route claims the whole
bracket interior, so a comment run there is emitted by it and nothing else — and a
run emitted with nothing between its comments welds into one, the second `//`
becoming text of the first. Each comment after a line comment takes a break, landing
at the bracket interior indent the expanded form already uses. Prettier keeps the run
distinct too (floating it past `]` to trail the statement), so it is the oracle for
the comments staying separate even though it is not the oracle for their placement.
The shared rule across every trailing gap is cataloged with
[retained_paren_shell_trailing_comment_run](../retained_paren_shell_trailing_comment_run_prettier_divergence/).

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
