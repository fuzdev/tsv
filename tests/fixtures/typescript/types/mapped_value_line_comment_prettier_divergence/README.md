# mapped_value_line_comment_prettier_divergence

A line comment after a mapped type's member `:`, before the value type
(`{ [K in T]: // c\n\t\tV }`).

**tsv** keeps the comment after `:`, with the value type on the next line.
**Prettier** breaks the `[K in T]` mapped-key brackets and trails the comment
after the key type, inside the brackets.

## Reason

Per Comment Position Philosophy: the user wrote the comment after the member
`:`, so tsv keeps it associated with the value rather than floating it past the
value to a member-trailing position. Both forms are idempotent in their
respective formatters. A same-line block comment (`[K in T]: /* c */ V`) stays
inline in both formatters and is not a divergence; only a line comment after `:`
diverges. Emitting the comment inline would swallow the value type onto the
comment line (a non-idempotent content loss), so tsv holds it on the `:` line
with the value on the next.

A redundant paren wrapping the value type with the line comment inside
(`[K in T]: (// c\n V)`, and the double-nested `((…))`) strips to this same fixed
point — the `unformatted_ours_single_paren` / `unformatted_ours_double_parens`
variants verify the paren form is idempotent too.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
