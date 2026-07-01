# predicate_is_own_line_block_comment_prettier_divergence

A single-line block comment in a type predicate's `is`→predicate-type gap collapses
to the inline form (`x is /* a */ T`) — whether authored glued, trailing `is`, or on
its own line. The own-line authoring (`x is⏎/* a */⏎T`,
`unformatted_ours_own_line.svelte`) is what tsv normalizes here.

- **tsv** collapses to `function f(x): x is /* a */ T` — the comment stays after `is`,
  on the predicate type.
- **Prettier** collapses *and relocates* the comment before `is` (`x /* a */ is T`).

Block comments inline losslessly, so neither formatter wraps; they differ only on
which side of `is` the comment lands for the own-line authoring. Per
[Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the predicate type
(after `is`) rather than floating it onto the parameter. Only a **line** comment
([predicate_is_line_comment](../predicate_is_line_comment_prettier_divergence/) —
prettier floats it to the body `{`) or a **multiline** block comment still hangs the
predicate type on its own line.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
