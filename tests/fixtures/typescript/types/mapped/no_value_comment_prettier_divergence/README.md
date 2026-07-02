# no_value_comment_prettier_divergence

A block comment trailing a mapped type **without a value type**, after the `]`
or the optional modifier (`{ [K in B] /* c */ }`, `{ [K in B]? /* c */ }`).

**tsv** keeps the comment where the author wrote it, trailing the member.
**Prettier** relocates it inside the brackets, after the key constraint
(`{ [K in B /* c */] }`, `{ [K in B /* c */]? }`).

## Reason

Per Comment Position Philosophy: the `]` (and the `?` modifier) is a semantic
boundary — a comment written after the whole member refers to the member, not
to the key constraint inside the brackets, so tsv holds it in place rather than
moving it across the bracket. Both forms are idempotent in their respective
formatters. With a value type present (`{ [K in B]: V /* c */ }`) the comment
trails the value in both formatters and there is no divergence; only the
no-value form diverges.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
