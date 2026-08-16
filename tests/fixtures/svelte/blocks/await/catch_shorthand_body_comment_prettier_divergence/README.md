# catch_shorthand_body_comment_prettier_divergence

A `{#await … catch e}` shorthand whose `{:then}` clause follows the catch body is
re-ordered to the canonical `{#await … then v}…{:catch e}…` form. A comment written in
the body that leads that re-order stays in that body, printed once. prettier-plugin-svelte
drops the trailing ones.

tsv: `{#await promise then value}…{:catch error}{error /* c */}{/await}` (preserved)
Prettier: `{#await promise then value}…{:catch error}{error}{/await}` (comment dropped)

`input.svelte` is the canonical authoring, where the surviving `then value` binding is
already in the head; `unformatted_ours_catch_shorthand.svelte` is the `catch`-shorthand
authoring of the same components, which tsv normalizes to it. The two differ only in
**where the head shorthand's binding pattern lives in source** — inside the head, or in a
`{:then}` clause after the body that prints last — which is the whole distinction this
fixture exists to hold: the awaited expression's comment range stops at the pattern
([destructure_comment](../destructure_comment_svelte_prettier_divergence/) pins why), and
a pattern outside the head would make that range span the earlier section's body, so the
range is bounded at the head's own `}` as well.

The `{/* c */ error}` case is the control: a block comment glued to its token is *owned*
by it, so the head's comment scan skips it on the emit axis and both formatters keep it.
Only the positions the head can also print — a trailing block, and every `//` — are at
stake, which is why the leading position looks healthy from either formatter's output.

`variant_catch_shorthand.svelte` is prettier's own output from the `catch`-shorthand
authoring: dual-stable, and one line shorter than `output_prettier.svelte` because the
dropped `//` no longer forces the fourth block to expand.

## Reason

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — the drop-vs-preserve rule for trailing comments in template expressions, across every `{…}` context
- [destructure_comment](../destructure_comment_svelte_prettier_divergence/) — the binding-pattern positions of the same heads
- [then_shorthand_catch](../then_shorthand_catch/) — the comment-free re-order this fixture's authoring pair is built on
