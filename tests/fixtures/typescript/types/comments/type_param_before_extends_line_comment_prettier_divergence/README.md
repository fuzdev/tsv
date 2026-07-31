# Divergence: type-parameter name→`extends` line comment (preserve, one indent level)

A line comment between a type parameter's name and its `extends` constraint
(`<T // c⏎extends A>`). A `//` runs to end-of-line, so the tail cannot stay on the
comment's line — inlining would swallow it (`T // c extends A`: the constraint
becomes comment text and the parameter silently loses its bound). tsv keeps the
comment where the author wrote it — trailing the name — and drops the whole
`extends A` tail to a continuation line at **one** indent level (uniform
forced-continuation indent). Prettier instead **relocates** the comment past
`extends` to lead the constraint, re-binding it from the name to the constraint.

```ts
// tsv (preserve + continuation)   // prettier (relocate past `extends`)
function fn<                       function fn<
	T // c                            T extends // c
		extends A                        A
>(): void {}                       >(): void {}
```

A stacked run stays with the name the same way (`T // c1⏎// c2⏎extends A`), and in
a multi-param list each run stays on its own parameter's name.

The type-parameter face of the before-keyword preserve rule: the same
one-indent-level continuation as the named-import-specifier `as` gaps
([specifier_as_gap_line_comment](../../../modules/imports/specifier_as_gap_line_comment_prettier_divergence/))
and the before-`:` key/binding gap
([binding_key_colon_line_comment](../../../declarations/variable/binding_key_colon_line_comment_prettier_divergence/)).
The same rule applies to Svelte `{#snippet}` generics
([ts_generic_constraint_gap_line_comment](../../../../svelte/blocks/snippet/ts_generic_constraint_gap_line_comment_prettier_divergence/)).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation)
and [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
