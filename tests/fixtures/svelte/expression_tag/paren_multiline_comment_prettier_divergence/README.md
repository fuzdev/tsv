# paren_multiline_comment_prettier_divergence

A **multi-line block comment leading a parenthesized expression** in an expression-tag
value — a plain template tag (`{/* c⏎*/ (a > b)}`) and a `style:` directive value
(`style:color={…}`, which Svelte models as an expression tag). The grouping parens are
redundant, so both formatters strip them — but tsv keeps the comment and expands to the
multi-line form (its stable, idempotent shape), while prettier **drops the comment** as
it strips the parens.

tsv (the `input.svelte` fixed point):

```svelte
<p>
	{/* c
	 */ a > b}
</p>
```

Prettier: `<p>{a > b}</p>` — the leading comment is lost with the parens.

The `unformatted_ours_paren.svelte` variant is the compact paren authoring tsv
normalizes to `input.svelte`; prettier does not (it drops the comment), so it carries
the divergence. `variant_comment_dropped.svelte` pins prettier's actual endpoint
(`{a > b}`, dual-stable), which tsv cannot recover the comment from once prettier has
run — preserving it up front is the only lossless path.

The **bare** authoring (no parens, `{/* c⏎*/ a > b}`) is **not** a divergence — there
the comment is glued to its operand and both formatters preserve it in the same expanded
form (the `input.svelte` shape). Only the paren-stripping path diverges: prettier
discards the comment along with the redundant parens, where tsv preserves it (a
multi-line block comment then forces the same break the bare form takes). This is the
expression-tag sibling of the directive-value case
[value_paren_multiline_comment](../../directives/value_paren_multiline_comment_prettier_divergence/);
both share the fix (a non-owned multi-line block leading comment reindented and
break-propagated like the owned path).

## Reason

User comments are valuable and shouldn't be silently removed; the comment is
syntactically valid here, and reproducing prettier's paren-stripped form would drop it.
tsv preserves the comment and reaches its stable expanded form on the first pass. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
