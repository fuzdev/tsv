# lifted_run_comment_whitespace_prettier_divergence

A `<script>` with a travelling comment on its own line above it (`<!-- c -->⏎<script>`),
written between two template nodes. The run goes to the top, and the neighbours join as if it
had never been there. The newline between the comment and the script is whitespace between the
neighbours too — the compiler renders `x<!-- c -->⏎<script>…</script>y` as `x y` — so the text
joins with a space. An authored blank line before or after the run keeps one blank line between
two elements or two expression tags; a single newline on each side is one line break. The
`unformatted_lifted_*` variants hold the authorings prettier also normalizes to `input.svelte`.

Prettier parts in two places:

- `x<!-- c -->⏎<script>…</script>y` (`unformatted_ours_lifted_text_run_newline`) comes out `xy`
  (`variant_space_dropped.svelte`): the newline in front of the script is deleted, and with it
  the rendered space.
- A blank line written above the comment, with one newline below the script
  (`<p>a</p>⏎⏎<!-- c -->⏎<script>…</script>⏎<p>b</p>`,
  `unformatted_ours_lifted_block_blank_before`), is dropped
  (`variant_blank_dropped.svelte`); written below the script it survives in both formatters.

## Reason

◆prettier_bug (the deleted space) and ◆stable_quirk (the blank line). See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
