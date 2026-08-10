# stripped_paren_interior_own_line_comment_prettier_divergence

A block comment written on its own line **inside** a stripped paren shell at a
binding default's tail (`a = (1⏎/* c */)`), across the object pattern, the array
pattern and a parameter list. Both formatters erase the shell, so the comment
lands in the list's own gap — and they hand it to different elements. tsv slides
it **forward** past the comma, leading the next element; prettier hoists it
**backward** to lead the element it was written in, across the `=` and the
binding name:

```ts
// tsv (forward, leads the next)   // prettier (backward, leads its own)
const { a1 = 1, /* c */ b1 } = x;  const { /* c */ a1 = 1, b1 } = x;
```

Both forms are dual-stable — the divergence is only which one the parenthesized
authoring normalizes to.

- `unformatted_ours_authored.svelte` — the authored shell form: tsv normalizes it
  to input, prettier to the variant.
- `unformatted_ours_next_own_line.svelte` — the same authoring with the next
  element pushed onto a line of its own. Where the author broke the line after
  the comma is layout, not own-line-ness, so both formatters land exactly where
  they land above.
- `variant_leading.svelte` — prettier's landing form, dual-stable: a comment
  *authored* leading an element keeps that position in both formatters.

tsv gives one answer wherever the shell sits; prettier changes its answer with
what the shell wraps. Where the shell wraps the **whole** element rather than a
default's value (`[(1⏎/* c */), 2]`, `fn((1⏎/* c */), 2)`) both formatters slide
the comment forward, exactly as tsv does here — so this row is prettier
switching direction for the `AssignmentPattern` shape alone, an artifact of
attaching the comment to that node and printing it at its front. Sliding
*forward* past re-emitted structure is lossless and is what every other tsv list
seam does; sliding *backward* across the `=` and the name is the relocation
[Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
declines — the same argument as the type-list sibling
[type_position_parens_glued_block_comment](../../../types/type_position_parens_glued_block_comment_prettier_divergence/),
where prettier likewise moves a shell-adjacent comment back across the comma.

The **same-line** spelling of this shell (`a = (1 /* c */)`) is a plain match,
pinned in [stripped_paren_interior_comment](../stripped_paren_interior_comment/)
and [param_default_stripped_paren_comment](../../../declarations/function/param_default_stripped_paren_comment/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
