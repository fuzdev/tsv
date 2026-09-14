# expr_leading_block_blank_prettier_divergence

The cells of [expr_leading_block_blank](./../expr_leading_block_blank/) where prettier parts:
an author blank line after a single-line block comment the author **broke after** in a
braced head's head→value gap. The break after the block is the author's (a block glued to its
value leads it inline; one with a newline after it ends its line — the separator follows the
source), and the blank that break separates is the author's too, so tsv keeps it at every
head, as it keeps the blank after a `//`
([expr_leading_blank](./../expr_leading_blank_prettier_divergence/)).

tsv:

```svelte
{#if /* c1 */

cond
}
```

Prettier keeps the blank at every value head (the plain sibling) and **drops** it at the
block heads (`{#if /* c1 */⏎cond}`, `{#each /* c2 */⏎items as item}`), where its second pass
then pulls the value up onto the comment's line (`{#if /* c1 */ cond}`, pinned by
`audit_signature.txt`); it strips the comment outright at `{@debug}`.

## Reason

A blank after a break the output keeps is the author's — the rule
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
states for a `//`'s forced break, at a break the author put after a block. One emitter answers
both comment kinds (the Svelte printer's `build_leading_js_comment_doc_before`), so a block
that ends its line and a `//` keep the blank below them the same way, and the block heads
answer as the value heads do rather than as prettier's block-head printer happens to.
Placement is [§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)'s:
a single-line block on the opener's line stays there in both formatters.

What the cases pin:

- **c1 / c2** — the `{#if}` and `{#each}` heads: prettier drops the blank.
- **c3** — `{@debug}`, whose own emitter keeps the blank; prettier strips the comment.
- **c4 / c5** — a block ahead of a `//`: the blank between them keeps its place, and the run
  and value hang one level in, the sibling sweeps' continuation indent.

See [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and [conformance_prettier_svelte.md §Svelte: Elements](../../../../../../docs/conformance_prettier_svelte.md#svelte-elements)
(the `@debug comments` entry, for c3).

## Related

- [expr_leading_block_blank](./../expr_leading_block_blank/) — the value heads, where both formatters keep the blank
- [expr_leading_blank](./../expr_leading_blank_prettier_divergence/) — the same blank after a `//`
- [own_line_leading_block_comment](./../own_line_leading_block_comment/) — the break after a block the author broke after, without the blank
