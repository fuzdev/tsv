# named_tuple_prettier_ignore_own_line_prettier_divergence

An own-line directive between a named tuple member's `label:` and its element type
freezes the element type — and tsv keeps the directive **own-line**, where the author
put it:

```ts
type A = [
	label:
		// prettier-ignore
		{x:   1},
	other: b
];
```

Prettier freezes the same span but **relocates** the directive to trail the label
(`label: // prettier-ignore`) and dedents the frozen slice
(`output_prettier.svelte`). tsv cannot adopt that form: a head-trailing directive is
**inert** under tsv's placement classification, so prettier's relocated form would
lose the freeze on the second pass — the authored own-line placement is the
idempotent fixed point (and the comment-position doctrine). Prettier's relocated form
is not even self-stable: its **own** second pass floats the directive past the member
(`label: { x: 1 }, // prettier-ignore`), losing the freeze (non-idempotent, pinned
via `audit_signature.txt`). The same rule at the annotation head:
[annotation_prettier_ignore_own_line](../annotation_prettier_ignore_own_line_prettier_divergence/).
`unformatted_ours_spaces.svelte` perturbs whitespace outside the frozen slice; tsv
normalizes it to input (prettier does not — it relocates the directive).

The glued and union-element placements match prettier — the ordinary
[named_tuple_prettier_ignore_element](../named_tuple_prettier_ignore_element/) fixture.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
