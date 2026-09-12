# conditional_prettier_ignore_check_paren_interior_prettier_divergence

An own-line directive the author wrote **inside the redundant paren shell of a conditional
type's check type** (`(⏎// prettier-ignore⏎{x:   1}⏎) extends Y ? a : b`). The shell is
redundant — `{x:   1} | {y:   2} extends Y ? a : b` parses with the union as the check type
either way — so it strips, the run keeps the line the author gave it in the enclosing
`=` gap, and the check type freezes, landing the shelled authoring on the bare authoring's
own fixed point in ONE pass:

```ts
type A =
	// prettier-ignore
	{x:   1} extends Y ? a : b;
```

`input.svelte` is that bare authoring, and both tools hold it: an own-line directive in the
alias `=`→value gap freezes the whole conditional value. `unformatted_ours_paren_shell.svelte`
is the shelled spelling of the same three cases.

The freeze scope over a **composite** check type is the whole check, operators and all
(cell B keeps both members verbatim) — prettier's scope here too, and what makes the two
authorings land on one form: a first-member-only freeze would normalize `{y:   2}` that the
bare authoring keeps.

## Why tsv differs

Prettier relocates the directive out of the shell to trail the alias `=`
(`type B = // prettier-ignore`) and dedents the frozen check to the head's indent. A
head-trailing directive is **inert** under tsv's placement classification, so that form
loses the freeze on tsv's second pass — which is also why prettier's own relocated form is
not self-stable: its second pass moves the directive back onto its own line and keeps the
conditional broken, so it never reaches `input.svelte`. That chain is pinned by
`audit_signature_paren_shell.txt` rather than by a single-form marker.

## Reason

◆comment_preservation ◆prettier_bug — the author's own-line placement is the only one that
holds the freeze across a second pass, and one authoring of a claim should not reach a
different fixed point than the other. Sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On single-child type positions*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
