# cast_target_leading_comment_prettier_divergence

A block comment in the leading gap of the required pair a type-assertion assignment
target prints — between the `(` and the target (`(/* c */ x as T) = 1;`).

**tsv**: keeps the block inside the pair, where the author wrote it — a glued block
leads the operand inline, an own-line block keeps its own line and expands the
shell:

```
(/* c1 */ x as T) = 1;
```

**Prettier**: hoists it out in front of the pair (`/* c1 */ (x as T) = 1;`),
re-binding it from the target to the whole statement.

## Reason

The line-comment sibling
([cast_target_leading_line_comment](../cast_target_leading_line_comment_prettier_divergence/))
carries the family argument: the pair is required, a comment inside it comments the
target, and tsv answers this gap the way it answers the non-null grouped operand's
(`(/* b */ x + y)!`). What the block cases add is the **run**: every block in a
leading run survives, not only the one glued to the operand — the others belong to
no node and reach no other emitter, so the gap must be emitted here.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
