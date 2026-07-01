# as_satisfies_value_own_line_block_comment_prettier_divergence

A single-line block comment in an `as`/`satisfies` cast keyword→type gap collapses to
the inline form (`x as /* a */ A`) — whether authored glued, trailing the keyword, or
on its own line. The own-line authoring (`x as⏎/* a */⏎A`,
`unformatted_ours_own_line.svelte`) is what tsv normalizes here.

- **tsv** collapses to `const a = x as /* a */ A` — the comment stays after the
  keyword, on the cast type, in one pass.
- **Prettier** collapses *and relocates* the comment before the keyword
  (`x /* a */ as A`), but reaches that form **non-idempotently**: its first pass
  leaves the comment after the keyword and hangs the type (`x as /* a */⏎A`), and
  only its second pass moves the comment across (`x /* a */ as A`). The two-pass
  chain is pinned here by `prettier_intermediate_to_variant_own_line.svelte` (the
  unstable first pass) and `variant_own_line.svelte` (the relocated fixed point,
  dual-stable in both formatters).

Block comments inline losslessly, so neither formatter wraps; they differ only on
which side of `as`/`satisfies` the comment lands for the own-line authoring. Per
[Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the cast type. Only a
**line** comment
([as_satisfies_value_line_comment](../as_satisfies_value_line_comment_prettier_divergence/) —
prettier floats it past the statement `;`) or an **own-line multiline** block
comment still hangs the cast type on its own line (a glued multiline block
collapses inline).

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
