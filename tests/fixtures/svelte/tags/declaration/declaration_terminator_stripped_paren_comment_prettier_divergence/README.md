# `{let}` / `{const}` ending on a comment from a stripped pair

The [declaration_terminator_comment](../declaration_terminator_comment_prettier_divergence/)
rule reached from inside the value: a comment the declaration ENDS on needs the `;` to
stand between it and the `}` (Svelte rejects `{let a = x || y /* c */}`), and a comment
inside a grouping pair the printer strips can end the declaration without being written
in its terminator gap.

- **tsv**: emits the `;` whenever the printed declaration ends on a comment — an own-line
  block glued onto the value's line (`c1`), and a hugged arrow's run that rides out past
  the call (`c2`, `c3`). A comment the value flushes inside itself — at a list's comma
  (`c4`), at a template interpolation's closer (`c5`) — ends nothing, and no `;` is
  emitted, as the bare control (`q`) gets none. The stripped-pair authoring
  (`unformatted_ours_inside_pair`) reaches `input.svelte` in one pass. Where the comment
  reaches the end only in some layouts, the render decides: see the `{let}` cells of
  [binary_stripped_operand_paren_line_comment_island_end_long](../../../expressions/binary_stripped_operand_paren_line_comment_island_end_long_prettier_divergence/).
  A same-line block the stripped pair leaves at the end takes the `;` too
  (`{let a = x || y /* c */;}`), pinned in `tests/declaration_tag_value_end_block_comment.rs`
  because prettier's first pass over that authoring is itself output Svelte rejects.
- **prettier**: from `input.svelte` moves each block past the `;` (`{let b = x || y; /*
  c1 */}`, which Svelte rejects) and the `//` past the `}` into page text
  (`{let d = f(() => x || y);} // c3`) — `output_prettier.svelte`, the terminator-gap bug
  of the sibling fixture. From the stripped authoring it moves the own-line comments out
  of the tag or into the call (`audit_signature_inside_pair.txt`).

See
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../../docs/conformance_prettier_svelte.md#svelte-attributes)
(the declaration tag's terminator) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
