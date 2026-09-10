# A detached terminator's block comment before a `case` label

A block comment between a case's last statement's content and that statement's own `;`,
where the next thing printed is a sibling `case` (or `default`) label. The terminator gap
belongs to the enclosing list rather than to the statement (prettier's `__contentEnd`), so
the comment is hoisted past the `;` on both formatters — the disagreement is only about the
LINE it lands on.

tsv applies its uniform rule: a comment glued to a pure separator shares the next item's
line (`/* c1 */ case 2:`). That is the same answer both formatters give in a plain statement
list — `fn10()⏎/* c */;⏎fn11();` prints as `/* c */ fn11();` on each — and the same answer
tsv gives at the clause-body seam. Prettier parts only here, because it attaches a comment
sitting before a `case` label BACKWARDS, to the consequent above it, and then dedents it on a
second pass.

- **tsv**: one pass to `/* c1 */ case 2:` from either authoring.
- **prettier**: two passes from the pre-`;` authoring — pass 1 leaves the comment at the
  CONSEQUENT's level (`prettier_intermediate_to_variant_pre_terminator.svelte`), pass 2
  re-reads it as the next case's leading run and dedents it to `variant_own_line.svelte`.

**Both landings are dual-stable and lossless**: each formatter keeps whichever of the two
forms it is handed, so `variant_own_line.svelte` is a `variant_*` rather than an
`output_prettier.*` — prettier keeps `input.svelte` unchanged. Nothing is merged, reordered
or dropped either way; the only difference is one line break.

The controls pin the boundary. The LAST case has no label to glue to, so the comment keeps
its own line on both. A statement FOLLOWING in the same consequent takes the glue at the
consequent's own level on both. And the run's last member glues even behind a `//` that
already ended its own line — the divergence is about the case label, not about the run.

Reason: the uniform separator rule, kept rather than special-cased at the one seam prettier
attaches backwards. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
