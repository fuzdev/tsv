# conditional_prettier_ignore_extends_prettier_divergence

An own-line directive between a conditional type's `extends` keyword and its extends
type freezes the extends type — and tsv keeps the directive **own-line**, where the
author put it:

```ts
type A = X extends
	// prettier-ignore
	{x:   1}
	? a
	: b;
```

Prettier instead glues the directive to trail the keyword (`X extends
// prettier-ignore`) and drops the frozen slice to the head's indent
(`output_prettier.svelte`) — and that relocated form is not even self-stable: its own
second pass reformats the type and floats the directive to trail it
(`X extends { x: 1 } // prettier-ignore` — freeze lost, non-idempotent, pinned via
`audit_signature.txt`). A keyword-trailing directive is inert under tsv's placement
classification, so the authored own-line placement is the only form that holds the
freeze — for the author, and across tsv's second pass.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
