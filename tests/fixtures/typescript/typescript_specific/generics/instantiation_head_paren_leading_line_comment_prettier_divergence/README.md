# instantiation_head_paren_leading_line_comment_prettier_divergence

A comment in the leading gap of the required pair an instantiation-expression head
prints — between the `(` and an arrow head (`( // c⏎<T>(p: T) => p)<string>;`).

**tsv**: keeps the comment inside the pair, on its authored line, and the pair
expands (a glued block stays flat and leads the head inline):

```
const i = ( // c1
	<T extends string>(p: T) => p
)<string>;
```

**Prettier**: hoists the comment out in front of the pair — hanging it in the
enclosing gap (`const i = // c1⏎(<T…>(p: T) => p)<string>;`), re-binding it from
the head to the whole statement.

## Reason

The same answer as the
[assignment-target shell](../../../expressions/assignment/cast_target_leading_line_comment_prettier_divergence/),
one construct over: the pair is required — a bare arrow cannot head an
instantiation — so it prints whatever the comment does, and a comment inside it
comments the head. A head whose pair is redundant strips it and the comment leads
the statement, matching prettier — the plain
[instantiation_head_paren_comment](../instantiation_head_paren_comment/) is that
control.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
