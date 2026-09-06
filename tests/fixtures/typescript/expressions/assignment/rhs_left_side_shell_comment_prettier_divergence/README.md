# rhs_left_side_shell_comment_prettier_divergence

The declarator rule
([init_left_side_shell_comment](../../../declarations/variable/init_left_side_shell_comment_prettier_divergence/))
at the other assignment seams: an assignment expression's RHS and a class field's
initializer, both laid out by the same `build_assignment_layout`. An own-line comment inside
the grouping parens of the value's **leftmost** node is hoisted ahead of the value's doc by
the value's printer, and the seam hangs the value under the operator so the run and the
value share the value's indent.

tsv, one pass (`input.svelte`):

```ts
x =
	// c
	(a as any) ? b : c;
```

Prettier takes two passes (`prettier_intermediate_to_variant_inner_shell.svelte` pins the first) and
lands with the comment on the operator's line, indenting the continuation — a form both
formatters keep (`variant_operator_line.svelte`):

```ts
x = // c
	(a as any) ? b : c;
```

Same reasoning as the declarator sibling. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment).
