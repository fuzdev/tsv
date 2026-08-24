# expression_trailing_comment_run_prettier_divergence

A line comment inside a decorator expression's parens
(`@(a ? b : c // inj⏎)`) with a second line comment trailing the decorator
itself (`) // c4`). The paren-gap comment defers past the `)` it cannot break,
so two comments reach the decorator line's end.

**tsv**: both keep their own line, in the authored order:

```
@(a ? b : c) // inj
// c4
class D {}
```

**prettier**: **welds** them — `@(a ? b : c) // inj // c4`, one comment whose
text contains the second, so `// c4` stops existing as a comment.

`input.svelte` is a fixed point for **both** formatters, and so is prettier's
welded landing (`variant_paren_comment.svelte` — tsv keeps the fused comment
verbatim, since it reparses as one); the divergence is entirely in how the
authored form normalizes, which is what
`unformatted_ours_paren_comment.svelte` states. An inline emission reaches
prettier's weld with the order **reversed** (`// c4 // inj`); the decorator's
trailing comment defers through the same `line_suffix` run, where the
flush's own separator breaks between the two (`doc/arena_render_suffix.rs`).

Reason: print-once over the weld, authored order preserved. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
