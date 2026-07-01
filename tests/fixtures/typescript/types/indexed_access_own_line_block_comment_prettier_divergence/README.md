# indexed_access_own_line_block_comment_prettier_divergence

A single-line block comment in an indexed access's `[`→index gap collapses to the
inline form (`A[/* c */ K]`) — whether authored glued, trailing `[`, or on its own
line. The own-line authoring (`A[⏎/* c */⏎K]`, `unformatted_ours_own_line.svelte`) is
what tsv normalizes here.

- **tsv** collapses to `type X = A[/* c */ K]` in **one pass** — the comment stays
  after `[`, inside the brackets leading the index.
- **Prettier** relocates the comment *out* of the brackets, before `[`
  (`A /* c */[K]`), but reaches it **non-idempotently**: its first pass emits
  `A[/* c */⏎K]` (comment glued to `[`, index dropped to the next line), and only its
  second pass lifts the comment out to `A /* c */[K]`.

Block comments inline losslessly, so neither formatter wraps; they differ only on
whether the comment stays inside the brackets (tsv, in place) or moves out before them
(Prettier, over two passes). tsv's one-pass in-place collapse is the better-behaved
form — it is the reason to diverge, not merely the position preference. Per [Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the index. Only a **line**
comment ([indexed_access_line_comment](../indexed_access_line_comment_prettier_divergence/))
or an **own-line multiline** block comment still hangs the index on its own line (a glued multiline block collapses inline).

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
