# html_render_paren_multiline_comment_prettier_divergence

A **multi-line block comment leading a parenthesized `{@html}` / `{@render}` value**
(`{@html /* c⏎*/ (foo)}`, `{@render /* c⏎*/ (foo())}`). The grouping parens are
redundant, so both formatters strip them — but tsv keeps the comment and reaches its
stable, idempotent expanded form, while prettier **drops the comment** as it strips the
parens.

tsv (the `input.svelte` fixed point):

```svelte
{@html /* c
 */ foo}
{@render /* c
 */ foo()}
```

Prettier: `{@html foo}` / `{@render foo()}` — the leading comment is lost with the parens.

The `unformatted_ours_paren.svelte` variant is the compact paren authoring tsv
normalizes to `input.svelte`; prettier does not (it drops the comment), so it carries
the divergence. `variant_comment_dropped.svelte` pins prettier's actual endpoint —
`{@html foo}` / `{@render foo()}` with the comment gone — which is dual-stable (both
formatters keep it), so tsv cannot recover the lost comment once prettier has run:
preserving it up front is the only lossless path.

The **bare** authoring (no parens, `{@html /* c⏎*/ foo}`) is **not** a divergence —
there the comment is glued to its operand and both formatters preserve it in the same
form, which is exactly `input.svelte` (`unformatted_bare.svelte`). Only the
paren-stripping path diverges: prettier discards the comment along with the redundant
parens, where tsv preserves it (a multi-line block comment then forces the same
reindent+break the bare form takes). This is the tag sibling of the directive-value and
expression-tag cases in
[value_paren_multiline_comment](../../directives/value_paren_multiline_comment_prettier_divergence/)
and
[paren_multiline_comment](../../expression_tag/paren_multiline_comment_prettier_divergence/).

## Reason

User comments are valuable and shouldn't be silently removed; the comment is
syntactically valid here, and reproducing prettier's paren-stripped form would drop it.
tsv preserves the comment and reaches its stable expanded form on the first pass. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
