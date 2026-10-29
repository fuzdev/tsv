# Member-access object to `.`, own-line block comment

A single-line block comment the author gave a line of its own between a member's object and
its `.` / `?.` (`a⏎/* c */⏎.b`), where nothing after the member is called. tsv keeps the
comment in the gap it was written in and glues it between the object and the member
(`a /* c */.b`): the member run prints flat, and a single-line block keeps its line only as
far as the enclosing layout keeps lines. The same at a statement, an initializer, an
assignment's right side and an arrow body, and for a multi-line block,
whose authored break after the `*/` is not kept at a member gap either. A member the chain's
head absorbs ahead of a call glues the same way (`a /* c */.b⏎\t.m(1);`).

Authored (`unformatted_ours_own_line.svelte`):

```ts
a
/* c */
.b;
const v1 = a
/* c */
?.b;
```

tsv (`input.svelte`), in one pass:

```ts
a /* c */.b;
const v1 = a /* c */?.b;
```

Prettier hoists the block out of the gap, ahead of the whole expression: above the statement
(`/* c */⏎a.b;`), and after the `=` of an initializer or an assignment, or the arrow's `=>`
(`const v1 =⏎\t/* c */⏎\ta?.b;`). For the head-absorbed member it glues `a/* c */` on its first pass and reaches
tsv's form on its second (`audit_signature_own_line.txt`).

A computed member keeps the block's own line before the `[`, and a `//` in this gap keeps its
line too (see the sibling
[computed_pre_bracket_block_comment](../computed_pre_bracket_block_comment_prettier_divergence/)).

Reason: comment position. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Member-access object to `.` own-line block comment) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
