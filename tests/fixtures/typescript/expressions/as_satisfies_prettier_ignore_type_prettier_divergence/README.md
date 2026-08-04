# as_satisfies_prettier_ignore_type_prettier_divergence

An own-line directive between an `as` (resp. `satisfies`) keyword and its type
freezes the type — and tsv keeps the directive **own-line**, where the author put
it:

```ts
const a = value as
	// prettier-ignore
	{x:   1};
```

Prettier's first pass relocates the directive to trail the keyword
(`value as // prettier-ignore`), still freezing the type (`output_prettier.svelte`)
— but that form is not self-stable: its own second pass reformats the type and
demotes the comment to trail the whole statement
(`const a = value as { x: 1 }; // prettier-ignore` — freeze lost, non-idempotent,
pinned via `audit_signature.txt`). A keyword-trailing directive is inert under tsv's
placement classification, so the authored own-line placement is the only form that
holds the freeze.

Hosts covered: `as` and `satisfies` — the same type position on each.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
