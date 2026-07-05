# interpolation_nested_template_prettier_divergence

An interpolation whose expression is an **array literal wrapping a long inner
template** — `${[`...`]}` — written **across lines** inside a deeply nested
code-generation template. Its collapsed form is 107 chars (over printWidth), so
the source newline makes tsv break the array to respect printWidth, while Prettier
keeps it inline (its `addAlignmentToDoc` reset measures the overflow as fitting).

The divergence is **authoring-triggered**: `variant_no_newline.svelte` is the same
document with the array written on one line (no newline in `${…}`). There both
formatters keep it inline at 107 chars — tsv preserves it exactly, matching
`output_prettier.svelte`. So tsv only wraps because the source spans lines; a
compact `${…}` is dual-stable.

The other nested-interpolation expression forms (ternary, logical, call, arrow
callback) at the same depth **match Prettier** and live in the non-divergence
[interpolation_nested_expr_long](../interpolation_nested_expr_long/) fixture. This
is the same deep-nesting layout edge as
[interpolation_nested_deep_long](../interpolation_nested_deep_long_prettier_divergence/)
(chains / ternaries); only the array-wrapping case is isolated here.

## Reason

Print width. A multi-line-source interpolation whose collapsed form overflows is
broken by tsv to respect printWidth at the true nested position; Prettier's
alignment reset lets the 107-char line render inline. With no newline the same
content stays on one line in both (see `variant_no_newline.svelte`).

See [conformance_prettier.md](../../../../../../../docs/conformance_prettier.md) §TypeScript: Template Literals.
