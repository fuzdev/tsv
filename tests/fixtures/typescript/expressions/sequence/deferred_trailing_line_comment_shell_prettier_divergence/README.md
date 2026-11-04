# Sequence trailing line comment inside the last operand's redundant pair

The shell twin of
[deferred_trailing_line_comment](../deferred_trailing_line_comment_prettier_divergence/): the
`//` sits inside a redundant grouping pair around the sequence's **last operand**
(`a, (b // c⏎)`), at any depth of nested sequences (`x = (a, (b, (c // c2⏎)))`) and down the
last operand's right spine (`a, b < (c // c3⏎)`). The pair strips, so the comment is the same
deferred run as the bare spelling's: nothing inside the sequence's parens ends a line after
it, so it renders after whatever the enclosing construct still prints on that line.

- **tsv**: one pass. The operand run stays on one line and the comment trails the
  statement (a declarator init too, `c6`), or the enclosing construct's own last line
  (`c4`, `c5`): the forms the bare spelling `(a, b // c⏎)` takes, which the shell spelling
  builds the same way. Inside a construct that continues past the sequence the comment
  ends its line there and the construct breaks after it, never carrying the comment out:
  an `if` / `while` / `switch` test, with or without the header's own pair doubled
  (`c7`–`c10`), drops its `)` below the comment, and a ternary's test takes its `?` on the
  next line (`c11`).
- **prettier**: its first pass splits the operand run one per line (`(a,⏎\tb); // c1`,
  `prettier_intermediate_shell`), and its second pass collapses it to `input.svelte`.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Sequence trailing line-comment convergence) and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).
