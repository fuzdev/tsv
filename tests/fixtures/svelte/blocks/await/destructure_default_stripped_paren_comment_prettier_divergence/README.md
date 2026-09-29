# Block pattern default ending on a comment from a stripped pair

A comment inside a grouping pair the printer strips from a destructuring **default value**
(`{ v = a || (b⏎/* c */⏎) }`) is deferred by the value's printer to wherever the line
ends. A block binding pattern stays on one line unless a comment breaks it, so that line
end is past the head's closing `}`: the comment would land in the block's body, rendered
there and read back on the next pass as body text.

- **tsv**: at `{#await … then}` (`c1`), `{:then}` (`c2`) and `{:catch}` (`c3`), the comment flushes where the default ends,
  glued onto its line (`{ v = a || b /* c1 */ }`) — the same-line block the next pass
  reads there, which keeps the pattern inline. So the stripped-pair authoring
  (`unformatted_ours_inside_pair`) reaches `input.svelte` in one pass. The bare control
  stays inline.
- **prettier**: throws on the input (`prettier_rejects.txt`): prettier-plugin-svelte's
  block-pattern printer has no case for a logical expression as a default value.

See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
