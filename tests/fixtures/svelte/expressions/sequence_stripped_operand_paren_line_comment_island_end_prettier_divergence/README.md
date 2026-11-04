# Sequence's last-operand paren, line comment, at a template island's end

The sequence twin of
[binary_stripped_operand_paren_line_comment_island_end](../binary_stripped_operand_paren_line_comment_island_end_prettier_divergence/),
and the island form of
[deferred_trailing_line_comment_shell](../../../typescript/expressions/sequence/deferred_trailing_line_comment_shell_prettier_divergence/):
a `//` inside a redundant grouping pair around a sequence's last operand, where nothing
inside the value ends a line after it — an expression tag, an attribute value, an `{#if}`
head and `{@html}`.

- **tsv**: one pass. The pair strips, the operand run stays on one line, the comment flushes
  where the value ends and the closer drops below it — the form the island twin's
  binary values take. The stripped-pair authoring (`unformatted_ours_inside_pair`)
  converges to it. Where the value continues past the sequence, as a ternary's test in an
  `{#if}` head does, the comment ends its line there and the `?` takes the next one, the
  bare spelling's form (`c5`).
- **prettier**: deletes a comment written after the value (`output_prettier.svelte`), except
  ahead of the ternary's `?`, where it moves the comment past the head's `}` as block body
  text, and its next pass indents it into the block (`audit_signature.txt`). From
  the stripped authoring its first pass splits the operand run and prints the comment past
  the closer — rendered page text, a stray in the attribute list, a block's body text — and
  its second pass collapses the run there; at the `{#if}` head it carries the comment on
  into the block body (`audit_signature_inside_pair.txt`).

See
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes)
(a `//` the value defers to its own end),
[conformance_prettier_ts.md §TypeScript](../../../../../docs/conformance_prettier_ts.md#typescript)
(Sequence trailing line-comment convergence) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
