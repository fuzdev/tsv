# expr_leading_line_paren_shell_cast_prettier_divergence

[expr_leading_line_paren_shell](./../expr_leading_line_paren_shell_prettier_divergence/)
reached through a **cast's operand**: the line comment sits in a redundant grouping paren on
the left spine of an `as` cast's operand, inside a second shell around the whole cast
(`title={((// c⏎a).b as T)}`). Both shells strip, and the operand prints first and bare, so
its spine hoists the run ahead of the whole value and it lands in the head→value gap. The
head owes it the same continuation indent as the paren-free authoring.

tsv, from either authoring:

```svelte
<div
	title={ // c
		a.b as T
	}
></div>
{#if // c
	a.b as T
}
```

`unformatted_ours_paren.svelte` is the shell authoring; tsv normalizes it to `input.svelte` in
one pass. Prettier keeps its own flush continuation (`title={// c⏎\ta.b as T}`,
`{#if // c⏎a.b as T}`), so the variant carries the divergence. The cases are a plain attribute
value (the unprefixed `{…}`) and a block head, two of the sibling's three resolving seams; the
`<script lang="ts">` is there because the cast needs TypeScript.

## Reason

The indent, the run's placement and the `}` column are all the twin fixture's, unchanged — see
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent),
whose **Svelte braced heads** entry carries this rule and the shell clause, the cast's operand
included.

## Related

- [expr_leading_line_paren_shell](./../expr_leading_line_paren_shell_prettier_divergence/) — the same shell without the cast, across the braced family
- [expr_leading_line](./../expr_leading_line_prettier_divergence/) — the plain gap, without the shell
