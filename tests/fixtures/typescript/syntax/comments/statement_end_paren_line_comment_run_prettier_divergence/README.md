# statement_end_paren_line_comment_run_prettier_divergence

A `//` written inside a stripped shell at the end of a statement's value, own-line before its
`)`, and a second `//` after the `)` (`const v = b && ([1]⏎// c⏎) // e`, the
`unformatted_ours_shell` authoring).

**tsv**: keeps each comment on a line of its own, in source order:

```ts
const v = b && [1];
// c
// e
```

**Prettier**: puts both on one line (`// c // e`), so `// e` stops being a comment of its own and
becomes text of `// c` — a content loss, and idempotent on its own output, so nothing reveals
it. That welded form is a stable form of tsv's too (`variant_shell.svelte`: one comment, kept as
written).

The statement's terminator gap takes over a value's trailing run only when the run is same-line
blocks; a `//` or an own-line comment keeps the deferral its own emitter gives it, which is what
keeps the two lines apart here.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
