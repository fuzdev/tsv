# keyword_own_line_block_comment_prettier_divergence

A single-line block comment in an `infer`→inferred-name gap collapses to the inline
form (`infer /* a */ R`) — whether authored glued, trailing `infer`, or on its own
line. The own-line authoring (`infer⏎/* a */⏎R`, `unformatted_ours_own_line.svelte`)
is what tsv normalizes here.

- **tsv** collapses to `type A = X extends infer /* a */ R ? R : never` in one pass —
  the comment stays after `infer`, on the inferred name.
- **Prettier** reaches the same inline form but is **non-idempotent** — its first pass
  pulls the comment onto the `infer` line yet leaves the name on the next line
  (`infer /* a */⏎R`), collapsing fully only on a second pass.

Block comments inline losslessly, so the collapse never drops information; `infer`,
like the prefix type operators, is an *in-place-collapse* gap (prettier keeps the
comment after `infer` rather than relocating it). Only a **line** comment (which
can't inline — [keyword_line_comment](../keyword_line_comment_prettier_divergence/))
or a **multiline** block comment still hangs the inferred name on its own line.
Covers the bare inferred name and the `extends`-constrained form.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
