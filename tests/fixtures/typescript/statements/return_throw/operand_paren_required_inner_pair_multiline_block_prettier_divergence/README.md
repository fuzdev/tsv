# Return/throw operand, required inner pair, multi-line block comment

The **stop condition** of
[operand_paren_leading_multiline_block_left_side](../operand_paren_leading_multiline_block_left_side_prettier_divergence/).
There the leftmost node's grouping shell is STRIPPED, so its run would land between the
keyword and its argument and the hanging parens are what hold it. Here that node's pair is
one the printer REQUIRES (`(a as any).b`, `(a++).b`): the comment is printed from inside
it and can never reach the keyword's line, so tsv adds no pair of its own.

tsv (`input.svelte`):

```ts
return (/* a
b */ a as any).b;
```

Prettier hoists the run out in front of the pair, re-binding it from the operand to the
statement — the cataloged relocation of
[required_pair_multiline_leading_comment](../../../syntax/comments/required_pair_multiline_leading_comment_prettier_divergence/),
reached here through a restricted production, where it is not merely a position change
(`output_prettier.svelte`):

```ts
return /* a
b */ (a as any).b;
```

That output **no longer parses as the input did**: a `MultiLineComment` holding a line
terminator IS one for ASI (ecma262 sec-comments), so the `return` ends at the comment and
the argument becomes a separate expression statement — prettier's own next pass writes the
split out (`return;`, pinned in `audit_signature.txt`), and at `throw` the same shape does
not parse at all.

Reason: content integrity, and one pair rather than two. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Return/throw hanging comment, left-side operand) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
