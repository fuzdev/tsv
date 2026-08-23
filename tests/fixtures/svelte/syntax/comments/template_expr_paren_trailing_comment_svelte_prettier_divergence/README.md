# A JSDoc cast at an island ROOT: the trailing comment matches, the leading one diverges

Two separate claims about the same shape, both stemming from `parse_expression_at`'s
`preserveParens: true` (`svelte/.../1-parse/acorn.js`), which gives acorn's walk a
`ParenthesizedExpression` root that tsv's tree does not have.

## The trailing comment — a MATCH, and what this fixture guards

acorn's comment window runs to the end of the trailing-comment run its tokenizer scans
past **after the parse**, and the parse ended at the cast's `)`. The comment then attaches
to the inner expression, because `)` is in acorn's own `/^[,) \t]*$/` trailing gap class:

```js
} else if (node.end <= comments[0].start && /^[,) \t]*$/.test(slice)) {
	node.trailingComments = [comments.shift()];
}
```

`remove_parens` discards the wrapper afterwards, so the attachment survives on the node
that replaces it — canonical emits `x.trailingComments = [" t1 "]`.

tsv anchors that scan on the **island's own expression** (`internal::JsdocCast`'s span
covers the `(`…`)`), not on the node the writer emits as the root — which for a cast is
the *inner* expression, stopping short of the `)`. Anchored there the scan dies on that
`)`, every comment past it is filtered out of the window before the walk runs, and the
attachment canonical emits has nowhere to land. `t1`–`t4` are that pin, at each position
where the cast is the island root (attribute value, `{#if}` head, `{@html}`) plus a
**nested** cast (`t4`, inside a call argument) where the root is the arrow and the anchor
never differed — the control that keeps this a claim about the root.

## The leading comment — the sanctioned divergence

The cast comment itself precedes the `(`, so acorn attaches it to the
`ParenthesizedExpression`, which `remove_parens` then throws away along with its
`leadingComments`. tsv has no such wrapper and attaches it to the inner expression
(`expected_ours.json` vs `expected_svelte.json`). Nothing is lost: it stays in the root
`comments` array in both parsers. This is the same divergence as the sibling
[template_expr_paren_comment](../template_expr_paren_comment_svelte_divergence/) fixture,
which isolates it with precedence parens instead.

**A bare grouping paren keeps the trailing bug**: `{(x) /* c */}` still loses the
attachment, because tsv discards ordinary grouping parens at parse time and keeps no span
to anchor on. It cannot be fixtured — neither formatter treats it as a fixed point (both
print `{x /* c */}`), which is also why it is unreachable from formatted code.

## Prettier

Prettier **deletes** a template JSDoc cast outright — the parens, the cast comment, and the
trailing comment with them (`{x}`) — at every position where the cast is the island root;
a nested one (`t4`) it leaves alone. That is the established behavior for this family, and
`output_prettier.svelte` records it.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences and
[conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).
