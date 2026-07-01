# heritage_keyword_own_line_block_comment_prettier_divergence

A single-line block comment after a heritage keyword (a class `extends`/`implements`
or an interface `extends`) collapses to the inline form (`class P extends /* a */ Q`)
— whether authored glued, trailing the keyword, or on its own line. The own-line
authoring (`extends⏎/* a */⏎Q`, `unformatted_ours_own_line.svelte`) is what tsv
normalizes here.

- **tsv** collapses to `class P extends /* a */ Q {}` — the comment stays after the
  keyword, on the heritage type.
- **Prettier** collapses *and relocates* the comment before the keyword
  (`class P /* a */ extends Q {}`).

Block comments inline losslessly, so neither formatter wraps; they differ only on
which side of the keyword the comment lands for the own-line authoring. Per
[Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the heritage type. Only a
**line** comment
([extends_keyword_line_comment](../extends_keyword_line_comment_prettier_divergence/))
or a **multiline** block comment still hangs the heritage type on its own line.
Covers class `extends`, class `implements`, and interface `extends`.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
