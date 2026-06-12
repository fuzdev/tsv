# mapped_value_line_comment_prettier_divergence

A line comment after a mapped type's member `:`, before the value type
(`{ [K in T]: // c\n\t\tV }`).

**tsv**: keeps the comment after `:`, with the value type on the next line:

```
type M = {
	[K in T]: // c
		V;
};
```

**Prettier**: relocates the comment to trail the member's `;`:

```
type M = {
	[K in T]: V; // c
};
```

## Reason

Per Comment Position Philosophy: the user wrote the comment after the member
`:`, so tsv keeps it associated with the value rather than floating it past the
value to a member-trailing position. Both forms are idempotent in their
respective formatters. A same-line block comment (`[K in T]: /* c */ V`) stays
inline in both formatters and is not a divergence; only a line comment after `:`
diverges.

Previously tsv emitted the comment inline and **swallowed the value type**
(`[K in T]: // c V;` — `V` absorbed into the comment, a non-idempotent content
loss); keeping it on the `:` line via `line_suffix` with the value on the next
line fixes the loss and preserves the user's placement.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
