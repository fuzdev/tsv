# type_predicate_prettier_ignore_type_prettier_divergence

An own-line directive between a type predicate's `is` and its type freezes the
predicate type — and tsv keeps the directive **own-line**, where the author put it:

```ts
function fn(a: unknown): a is
	// prettier-ignore
	{x:   1} {
	return true;
}
```

Prettier's first pass relocates the directive to trail the `is`
(`a is // prettier-ignore`), still freezing the type (`output_prettier.svelte`) —
but the chain doesn't stop there: its second pass loses the freeze and glues the
comment to trail the function body's `{`, and its third pass moves it to lead
`return` inside the body, where it finally stabilizes (non-idempotent across three
distinct forms, pinned via `audit_signature.txt`). An `is`-trailing directive is
inert under tsv's placement classification, so the authored own-line placement is
the only form that holds the freeze.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
