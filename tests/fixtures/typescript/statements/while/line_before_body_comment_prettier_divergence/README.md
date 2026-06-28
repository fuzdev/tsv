# line_before_body_comment_prettier_divergence

Line comments between a `while (…)` header's `)` and the body `{`.

Prettier 3.9 no longer absorbs these comments into the block body — a trailing
comment (`while (a) // c`) and an own-line comment before `{` (`while (a)\n// c\n{`)
now stay where the author wrote them in both formatters. The remaining divergence
is the **blank line**: when the author leaves a blank line between `)` and an
own-line comment, tsv preserves it; prettier 3.9 drops it.

```ts
// prettier 3.9 (blank dropped)   // tsv (blank preserved)
while (a)                          while (a)

// blank before                   // blank before
{                                  {
	fn();                              fn();
}                                  }
```

## Reason

tsv treats the author's vertical spacing as intentional, preserving the blank
line before the comment. Consistent with tsv's comment-position handling across
control-flow statements.

`variant_spaces.svelte` is a dual-stable form prettier reaches from the
extra-whitespace `unformatted_ours_spaces.svelte` (it keeps a blank line *after*
the comment instead); `unformatted_ours_*` variants normalize back to input under
tsv only.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
