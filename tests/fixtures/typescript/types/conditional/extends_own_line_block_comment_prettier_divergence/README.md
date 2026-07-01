# extends_own_line_block_comment_prettier_divergence

A single-line block comment in a conditional type's `extends`→extends-type gap
collapses to the inline form (`X extends /* a */ Y ? T : F`) — whether authored
glued, trailing `extends`, or on its own line. The own-line authoring
(`X extends⏎/* a */⏎Y`, `unformatted_ours_own_line.svelte`) is what tsv normalizes
here.

- **tsv** collapses to `type A = X extends /* a */ Y ? T : F` — the comment stays
  after `extends`, the conditional inline, in one pass.
- **Prettier** collapses *and relocates* the comment before `extends`
  (`X /* a */ extends Y ? T : F`), but reaches that form **non-idempotently**: its
  first pass leaves the comment after `extends` and hangs the check type
  (`X extends /* a */⏎Y`), and only its second pass moves the comment across. The
  two-pass chain is pinned here by `prettier_intermediate_to_variant_own_line.svelte`
  (the unstable first pass) and `variant_own_line.svelte` (the relocated fixed point,
  dual-stable in both formatters).

Block comments inline losslessly, so neither formatter wraps; they differ only on
which side of `extends` the comment lands for the own-line authoring. Per
[Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the extends-type. Only a
**line** comment
([check_extends_line_comment](../check_extends_line_comment_prettier_divergence/) —
prettier relocates it to trail the extends-type) or a **multiline** block comment
still hangs the extends-type on its own line and forces the conditional to break.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
