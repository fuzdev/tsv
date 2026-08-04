# type_param_prettier_ignore_value_prettier_divergence

An own-line directive before a type parameter's `extends` constraint or `=` default
freezes the value that follows — and tsv keeps the directive **own-line**, where the
author put it:

```ts
type A<
	T extends
		// prettier-ignore
		{x:   1}
> = T;
```

Prettier freezes the same span but **relocates** the directive to trail the head
(`T extends // prettier-ignore` / `T = // prettier-ignore`) and dedents the frozen
slice (`output_prettier.svelte`). tsv cannot adopt that form: a head-trailing
directive is **inert** under tsv's placement classification, so prettier's relocated
form would lose the freeze on the second pass — the authored own-line placement is
the idempotent fixed point (and the comment-position doctrine). Prettier's relocated
form is not even self-stable: its **own** second pass reformats every relocated
constraint/default (`{x:   1}` → `{ x: 1 }`), losing the freeze (non-idempotent,
pinned via `audit_signature.txt`).

Controls: a directive **trailing** `extends` is inert in both tools (the constraint
formats normally — `unformatted_ours_perturbed.svelte` proves it by perturbing that
constraint's interior, which tsv normalizes); a **union** constraint freezes only its
first member (Rule A), where prettier agrees and does not relocate.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
