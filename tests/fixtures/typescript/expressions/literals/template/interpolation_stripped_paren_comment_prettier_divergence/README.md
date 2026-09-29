# Stripped operand paren, line comment, inside a template interpolation

A `//` inside a grouping pair the printer strips, in a `${…}` whose value is followed by a
comment the author wrote after it. The pair's comment is deferred to wherever the value's
line ends, while the written one prints inline behind the value — so left to the
interpolation's closing break, the deferred one lands BEHIND the written one on the same
line: `x || y // d1 // c1`, where `// c1` is no longer a comment of its own but the text of
`// d1`, and the authored order is gone.

- **tsv**: the deferred comment flushes where the value ends, ahead of the written run,
  which follows on the next line (`c1`, `d1`) — or behind a block on the next line too
  (`c3`, `d3`) — the form the written trailing run takes when the author writes both
  comments after the value
  ([interpolation_comment_trailing_run](../interpolation_comment_trailing_run/)). A sole
  hugged arrow's run rides out past the call and flushes there (`c2`). With no comment
  after the value the closer's own break takes it (`c4`) — and where the interpolation
  hugs its value rather than wrapping it in breaks, a hugged arrow's call breaks and takes
  the comment inside (`c6`, `c7`), as with no pair at all. The stripped-pair authoring
  (`unformatted_ours_inside_pair`) reaches `input.svelte` in one pass, in a `<script>` and
  in a template island (`c5`) alike.
- **prettier**: welds the two into one comment (`y // c1 // d1`), and reorders a block
  behind the deferred `//` (`y /* d3 */ // c3`) — `audit_signature_inside_pair.txt`.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
