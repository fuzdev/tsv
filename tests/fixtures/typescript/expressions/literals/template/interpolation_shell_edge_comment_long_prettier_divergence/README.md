# Template interpolation, comment in a stripped shell, at the print-width boundary

The width face of
[interpolation_shell_edge_comment](../interpolation_shell_edge_comment_prettier_divergence/):
a call value whose callee's stripped shell holds a comment (`${(/* c1 */ fff)(…)⏎}`), its source
spanning lines. At 100 columns the interpolation fits and stays on one line in both formatters
(`c1`). At 101 it breaks at its `${` and `}`, the comment leading the value on its own line
(`c2`) — what both formatters print for the comment written outside the shell.

- **tsv**: one pass from the authored form (`unformatted_ours_shell`).
- **prettier**: its first pass misses the comment, keeps the `${` hugged and breaks the call's
  arguments instead (`prettier_intermediate_shell.svelte`); its second breaks at the `${`.

See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
