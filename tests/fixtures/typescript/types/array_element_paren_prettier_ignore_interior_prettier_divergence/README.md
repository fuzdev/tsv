# array_element_paren_prettier_ignore_interior_prettier_divergence

An own-line directive inside a **required** paren (an array type's function-type
element, where the paren must survive) freezes the inner function type only — Rule
A's child scope: the directive targets the construct it precedes. tsv keeps the
directive own-line inside the parens and re-synthesizes the paren and `[]` outside
the frozen slice:

```ts
type A = (
	// prettier-ignore
	(a:  string) => void
)[];
```

Prettier freezes a **coarser** unit — the whole `((a:  string) => void)[]` — and
relocates the directive out of the parens to trail the alias `=`
(`type A = // prettier-ignore`, `output_prettier.svelte`); its own second pass moves
the directive own-line under the `=`, where it stabilizes with the freeze intact
(2-pass, pinned via `audit_signature.txt`). Both the scope and the placement
diverge: the directive precedes the inner function type, not the enclosing array
type, so tsv freezes the child it points at and keeps the authored in-paren
own-line position per the comment-position doctrine.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
