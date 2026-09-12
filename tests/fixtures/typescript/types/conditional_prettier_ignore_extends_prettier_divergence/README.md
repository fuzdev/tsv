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

`unformatted_ours_paren_shell.svelte` writes the directive **inside** the extends type's
redundant paren shell (`X extends (⏎// prettier-ignore⏎…⏎)`): the shell strips, the run keeps
its own line and the inner freezes, so the shelled authoring converges in one pass onto the
bare authoring's fixed point — one fixed point per formatter, not per authoring. Without it
the run was relocated past the whole operand to trail it (`X extends { x: 1 }
// prettier-ignore`), the inert placement the bare form's relocation also lands on. Prettier
strips the shell too, with the same relocation it applies to the bare form, so it converges
onto `output_prettier.svelte`; the variant is `_ours_` because only tsv lands on `input`.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
