# constraint_default_own_line_block_comment_prettier_divergence

A single-line block comment in a type parameter's `extends` constraint or `=`
default gap collapses to the inline form (`<T extends /* a */ U>`, `<T = /* b */ U>`)
and keeps the `<…>` list collapsed — whether authored glued, trailing the keyword, or
on its own line. The own-line authoring (`<T extends⏎/* a */⏎U>`,
`unformatted_ours_own_line.svelte`) is what tsv normalizes here.

- **tsv** collapses to `type H<T extends /* a */ U> = T` — the comment stays after the
  keyword, `<…>` inline, in one pass.
- **Prettier** collapses *and relocates* the comment before the keyword
  (`<T /* a */ extends U>`), but reaches that form **non-idempotently**: its first
  pass leaves the comment after the keyword and hangs the bound type
  (`<T extends /* a */⏎U>`), and only its second pass moves the comment across. The
  two-pass chain is pinned here by `prettier_intermediate_to_variant_own_line.svelte`
  (the unstable first pass) and `variant_own_line.svelte` (the relocated fixed point,
  dual-stable in both formatters).

Block comments inline losslessly, so neither formatter wraps or expands the `<…>`;
they differ only on which side of `extends`/`=` the comment lands for the own-line
authoring. Per [Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the bound type. Only a
**line** comment or an **own-line multiline** block comment still expands the `<…>`
and hangs the bound type on its own line (a glued multiline block collapses inline
and keeps `<…>` collapsed). Covers the `extends` constraint and the `=` default.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
