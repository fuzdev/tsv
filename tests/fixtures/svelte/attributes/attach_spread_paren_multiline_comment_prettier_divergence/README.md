# attach_spread_paren_multiline_comment_prettier_divergence

A **multi-line block comment leading a parenthesized `{@attach}` / `{...spread}` value**
(`{@attach /* c⏎*/ (foo)}`, `{.../* c⏎*/ (foo)}`). The grouping parens are redundant, so
both formatters strip them — but tsv keeps the comment and reaches its stable, idempotent
form (the comment forces the attribute onto its own line), while prettier **drops the
comment** as it strips the parens.

tsv (the `input.svelte` fixed point):

```svelte
<div
	{@attach /* c
	 */ foo}
></div>
<div
	{.../* c
	 */ foo}
></div>
```

Prettier: `<div {@attach foo}></div>` / `<div {...foo}></div>` — the leading comment is
lost with the parens (and, comment gone, the attribute fits inline).

The `unformatted_ours_paren.svelte` variant is the compact paren authoring tsv
normalizes to `input.svelte`; prettier does not (it drops the comment), so it carries
the divergence. `variant_comment_dropped.svelte` pins prettier's actual endpoint — the
comment gone — which is dual-stable (both formatters keep it), so tsv cannot recover the
lost comment once prettier has run: preserving it up front is the only lossless path.

The **bare** authoring (no parens, `{@attach /* c⏎*/ foo}`) is **not** a divergence —
there the comment is glued to its operand and both formatters preserve it in the same
expanded form, which is exactly `input.svelte` (`unformatted_bare.svelte`). Only the
paren-stripping path diverges. Both `{@attach}` and the `{...spread}` attribute share one
emitter (`build_braced_expression_doc`), so they cannot drift. This is the
attribute-brace sibling of the directive-value and expression-tag cases in
[value_paren_multiline_comment](../../directives/value_paren_multiline_comment_prettier_divergence/)
and
[paren_multiline_comment](../../expression_tag/paren_multiline_comment_prettier_divergence/).

## Reason

User comments are valuable and shouldn't be silently removed; the comment is
syntactically valid here, and reproducing prettier's paren-stripped form would drop it.
tsv preserves the comment and reaches its stable expanded form on the first pass. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
