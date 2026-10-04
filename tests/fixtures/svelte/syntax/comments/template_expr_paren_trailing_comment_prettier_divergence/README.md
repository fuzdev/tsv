# A JSDoc cast at an island ROOT: both of its comments attach as Svelte attaches them

Svelte parses every template expression with `parse_expression_at`'s `preserveParens: true`
(`svelte/.../1-parse/acorn.js`), which gives acorn's walk a `ParenthesizedExpression` root
that `remove_parens` then discards, together with every comment it claimed. tsv runs the
cast's parens as that same discarded node, so the parser side is a **match** on both sides
of the cast — this fixture pins it; the divergence is prettier's.

## The trailing comment

acorn's comment window runs to the end of the trailing-comment run its tokenizer scans
past **after the parse**, and the parse ended at the cast's `)`. The comment then attaches
to the inner expression, because `)` is in acorn's own `/^[,) \t]*$/` trailing gap class:

```js
} else if (node.end <= comments[0].start && /^[,) \t]*$/.test(slice)) {
	node.trailingComments = [comments.shift()];
}
```

`remove_parens` discards the wrapper afterwards, so the attachment survives on the node
that replaces it — `x.trailingComments = [" t1 "]`. The scan has to start past the `)`:
anchored on the inner expression, it stops dead on that `)` and every comment past it is
filtered out of the island's window before the walk runs. `t1`–`t4` pin it at each
position where the cast is the island root (attribute value, `{#if}` head, `{@html}`) plus
a **nested** cast (`t4`, inside a call argument), where the root is the arrow.

## A trailing RUN — only its first comment attaches

acorn's walk makes a single trailing claim, so a second comment in the run (`/* a */ /* b */`,
or `// a⏎// b⏎` before a block head's `}`) is left over. Svelte's post-walk fallback then
hands every leftover comment past the root's end to the root — but the root it holds is the
`ParenthesizedExpression`, and `remove_parens` discards it with its `trailingComments`. So at
a parenthesized island root the run's later comments attach **nowhere**:
`x.trailingComments = [" t5 "]`, never `[" t5 ", " t6 "]`. `t5`–`t20` pin it at every
position a cast can be the island root: attribute value, a `{#if}` head closed by a run of
`//` (`t7`, `t8`), `{@html}`, `bind:value`, a spread, the `{#each}` iterable, a `{@const}`
init, and `{@render}`, whose cast wraps the whole call.

## The leading comment

The cast comment itself precedes the `(`, so acorn attaches it to the
`ParenthesizedExpression`, which `remove_parens` throws away along with its
`leadingComments`: it attaches nowhere, on both sides, and stays in the root `comments`
array.

**A bare grouping paren around the root** answers the same way — `{(x) /* c */}` attaches
`c` to `x`, a run past it keeps only its first comment, and a comment before its `(` attaches
nowhere — but it cannot be fixtured: neither formatter treats it as a fixed point (both print
`{x /* c */}`), which is also why it is unreachable from formatted code. Its cells, and the
newline-separated forms no fixture can hold either, are pinned in
[svelte_paren_root_trailing_comment_attach.rs](../../../../../svelte_paren_root_trailing_comment_attach.rs).
A bare paren **inside** an expression is the residual the sibling
[template_expr_paren_comment](../template_expr_paren_comment_svelte_divergence/) pins.

## Prettier

Prettier **deletes** a template JSDoc cast outright — the parens, the cast comment, and the
trailing comment with them (`{x}`) — at every position where the cast is the island root;
a nested one (`t4`) it leaves alone. That is the established behavior for this family, and
`output_prettier.svelte` records it. The `{@const}` init is the exception: there prettier
keeps the cast and the run but emits an unmatched `)` (`(x)) /* t17 */ /* t18 */`) — the
corrupt `{@const}` output this family already records, pinned verbatim as in
[expr_trailing_multiline](../expr_trailing_multiline_prettier_divergence/). tsv rejects that
document, so `scripts/check_loc_rejects.txt` lists it.

See [conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics)
and [conformance_svelte.md](../../../../../../docs/conformance_svelte.md#comment-attachment-differences)
§Comment Attachment Differences.
