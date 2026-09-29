# Stripped operand paren, line comment, at a template island's end

The island twin of
[stripped_paren_trailing_line_comment](../../../typescript/syntax/comments/stripped_paren_trailing_line_comment_prettier_divergence/),
at the position its list sibling
[binary_stripped_operand_paren_line_comment](../binary_stripped_operand_paren_line_comment_prettier_divergence/)
does not reach: a `//` inside a grouping pair the printer strips, where nothing inside the
value ends a line after it — an expression tag, an attribute value, an `{#if}` head, an
`{#each}` head before its `as`, `{@const}`, a `{let}` declaration tag, `{@html}`, a `bind:`
function sequence, a curried `{@const}` stacked under a leading `//`, and a sole arrow
argument the call keeps hugged.

- **tsv**: one pass. The comment flushes where the value ends and the closer drops below
  it. `input.svelte` is that form written directly — the comment after the value, no pair
  — and was already its own fixed point; the stripped-pair authoring
  (`unformatted_ours_inside_pair`) and the own-line one (`unformatted_ours_own_line`)
  converge to it. A comment the author wrote after the pair follows the flushed one on the
  next line (`c9`), in authored order, and the closer's separator is shed behind the break
  — the space before `as` (`c10`), the host's own break before `}`. The `<script>` cell
  (`c0`) is the control: there the statement's own end takes the comment. A pair followed
  by more of the value strips as it does in a `<script>`, its comma ending the line
  (`c12`) — and ahead of a comment the author wrote at that comma, which follows on the
  next line (`c14`). A run that reaches the value's end only in some layouts leaves the
  host's layout alone and lets the render settle it (`c15`: the member chain stays on the
  value's line).
- The **hugged arrow** cells (`c8`, `c10`) are the `<script>` rule's own exception: the
  sole arrow's run trails past the call's `)`, so the island flushes it after the call —
  where the paren-free authoring (`{f(() => x + y // c⏎)}`) keeps the comment inside the
  call instead (`{f(⏎\t\t() => x + y // c⏎\t)}`). The two authorings do not converge, as
  they do not in a `<script>`.
- **prettier**: deletes a comment written after the value (`output_prettier.svelte`), and
  also moves `c5` and `c13` past the `}` leaving an unmatched `)` (`{@const r = x + y * z)}
  // c5`), and `c6` past the `}` into page text (`{let s = x + y * z;} // c6`). From the
  stripped authoring its first pass prints the comment past the closer — rendered page
  text, a stray in the attribute list, a block's body text
  (`audit_signature_inside_pair.txt`, `audit_signature_own_line.txt`); only at the block
  heads (`c3`, `c4`, `c10`) does its second pass carry it on into the block body, and
  everywhere else it stays past the closer.
- The own-line cells glue onto the value's line: the flushed comment lands where a
  comment the author wrote after the value lands, and `c0` stays same-line in the own-line
  variant because the `<script>` statement's own-line spelling re-indents on its next pass
  (a separate `<script>` issue).

See
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes)
(a `//` the value defers to its own end),
[conformance_prettier_ts.md §TypeScript](../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence),
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).
