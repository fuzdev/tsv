# mapped_value_prettier_ignore_own_line_prettier_divergence

An own-line directive between a mapped type's `]:` and its value type freezes the
value type — and tsv keeps the directive **own-line**, where the author put it:

```ts
type A = {
	[K in keyof T]:
		// prettier-ignore
		{x:   1};
};
```

Prettier freezes the same span but **relocates** the directive to trail the head
(`[K in keyof T]: // prettier-ignore`) and dedents the frozen slice
(`output_prettier.svelte`). tsv cannot adopt that form: a head-trailing directive is
**inert** under tsv's placement classification, so prettier's relocated form would
lose the freeze on the second pass — the authored own-line placement is the
idempotent fixed point (and the comment-position doctrine). Prettier's relocated form
is not even self-stable: its **own** second pass moves the directive *inside* the
bracket (`[K in keyof T // prettier-ignore]`) and reformats the value, losing the
freeze (non-idempotent, pinned via `audit_signature.txt`). The same rule at the
annotation head:
[annotation_prettier_ignore_own_line](../annotation_prettier_ignore_own_line_prettier_divergence/).

Control: a **union** value freezes only its first member (Rule A), where prettier
agrees and does not relocate. `unformatted_ours_spaces.svelte` perturbs whitespace
outside the frozen slices; tsv normalizes it to input (prettier does not — it
relocates the non-union directive).

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
