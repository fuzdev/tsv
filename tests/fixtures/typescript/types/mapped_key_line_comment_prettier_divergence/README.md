# mapped_key_line_comment_prettier_divergence

A line comment trailing the mapped-type key constraint, *before* the `]`
(`{ [K in T // c]: V }`).

**tsv** drops the `]` to its own line so the `//` can't swallow it, keeping
`K in T` on the `[` line. **Prettier** also breaks the `[K in T]` brackets but
additionally drops `K in T` to its own indented line, leaving `[` alone.

The divergence is the bracket layout only (tsv keeps `K in T` on the `[` line;
prettier breaks after `[`); both keep the comment trailing the key and the `]`
on its own line. The load-bearing part is that `]` is **never** emitted inline
after the `//` — doing so swallows it (a non-idempotent content loss). This is
the key-side counterpart of
[mapped_value_line_comment](../mapped_value_line_comment_prettier_divergence/)
(the comment after `:`).

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
