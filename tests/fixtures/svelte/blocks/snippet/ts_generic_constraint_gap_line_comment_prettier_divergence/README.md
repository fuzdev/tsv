# Divergence: `{#snippet}` generic name→`extends` line comment (preserve, one indent level)

The Svelte `{#snippet}` face of the type-parameter name→`extends` line-comment
divergence: a line comment between a snippet generic's name and its `extends`
constraint stays trailing the name, and the `extends X` tail continues one indent
level in; prettier relocates the comment past `extends` to lead the constraint.
The snippet's generics route through the same TypeScript type-parameter printer
as every other context (context-free TS formatting), so the rule and rendering
are identical to
[type_param_before_extends_line_comment](../../../../typescript/types/comments/type_param_before_extends_line_comment_prettier_divergence/).

```svelte
<!-- tsv (preserve + continuation)   prettier (relocate past `extends`) -->
{#snippet fn<                        {#snippet fn<
	T // c                               T extends // c
		extends X                           X
>(a: T)}                             >(a: T)}
```

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation)
and [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
