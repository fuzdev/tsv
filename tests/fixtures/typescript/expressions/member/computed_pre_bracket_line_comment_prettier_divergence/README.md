# computed_pre_bracket_line_comment_prettier_divergence

A line comment in the gap between a computed access's **object and its `[`**
(`arr // c⏎[i]`) — the computed-member counterpart of
[member_only_interior_line_comment](../../calls/chained/member_only_interior_line_comment_prettier_divergence/),
which covers the same gap on a plain `.prop` member. The **numeric-index** form of the
same gap (`foo.bar().baz()⏎// c⏎[0]`, where prettier has no fixed point at all) is
pinned separately by
[computed_numeric_index_pre_bracket_line_comment](../computed_numeric_index_pre_bracket_line_comment_prettier_divergence/).

**tsv** breaks the chain at every bracket and keeps each comment where the author
wrote it: a same-line comment trails the object (`arr // c1`), an own-line comment
keeps its own line before the `[`. A comment written *inside* the brackets belongs to
the bracket, not to the chain gap, so it stays inside them (case `d`, a chain with a
call — the path that reaches the brackets through the chain builder).

**Prettier** hoists the comment out to lead the whole expression, before the
assignment RHS — and when two comments share the gap it **reorders** them
(`// c3` above `// c2`, case `b`), because the own-line comment is hoisted past
the same-line one.

```
// tsv                     // prettier
const a = arr // c1        const a =
	[i];                       arr // c1
                               [i];
```

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
rather than hoisting it out to the whole expression. A `//` must end its line, so
the bracket drops to the next line — otherwise the comment would swallow `[i]`.
Preserving also keeps two comments in the same gap **distinct and in order**;
prettier's hoist reorders them (case `b`).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
