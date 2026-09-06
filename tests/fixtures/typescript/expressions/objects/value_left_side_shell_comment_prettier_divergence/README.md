# value_left_side_shell_comment_prettier_divergence

An own-line comment inside the grouping parens of an object property value's **leftmost**
node (`k: (⏎// c⏎a as any) ? b : c`). The value's printer hoists the run ahead of its doc,
and the `:`→value seam hangs the value under the key so the run and the value share the
value's indent — the comment stays on the value's side of the `:`, where the author put it
(`input.svelte`):

```ts
k:
	// c
	(a as any) ? b : c,
```

Prettier takes two passes from the authored form (`prettier_intermediate_to_variant_inner_shell.svelte`
pins the first: the comment flush on the key's line, the conditional broken by the hardline
it carries) and lands with the comment hoisted **before the key** — the relocation of
[property_key_colon_line_comment](../property_key_colon_line_comment_prettier_divergence/),
reached from a comment that started inside the value. That landing is a fixed point for
both formatters (`variant_before_key.svelte`); tsv converges to its own in one pass.

The property face of the declarator's
[init_left_side_shell_comment](../../../declarations/variable/init_left_side_shell_comment_prettier_divergence/).
See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment).
