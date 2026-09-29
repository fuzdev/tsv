# Stripped operand paren, line comment, inside a computed bracket

A `//` inside a grouping pair the printer strips, in a computed member's index or a
computed object key, whose value is followed by a comment the author wrote before the `]`.
The pair's comment is deferred to wherever the key's line ends, while the written one
prints inline behind the key — so left to the `]`'s break, the deferred one lands BEHIND
the written one on the same line (`a || b // d1 // c1`), where the two weld into one
comment and the authored order is gone.

- **tsv**: the deferred comment flushes where the key ends, ahead of the written one,
  which follows on the next line (`c1`/`d1` in a member index, `c2`/`d2` in an object
  key, `c3`/`d3` in a template island). The stripped-pair authoring
  (`unformatted_ours_inside_pair`) reaches `input.svelte` in one pass.
- **prettier**: welds the two into one comment in the member index (`b // c1 // d1`), and
  relocates both out of the object key's brackets (`audit_signature_inside_pair.txt`),
  the computed-key relocation cataloged in
  [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
