# branch_paren_leading_line_comment_weld_prettier_divergence

The branch-position twin of
[extends_paren_line_comment_weld](../extends_paren_line_comment_weld_prettier_divergence/).
A redundant paren shell around a conditional **branch** whose leading gap holds a line
comment relocates to trail the node before the operator — the extends-type for the `?` arm,
the true branch for the `:` arm. That is lossless only while the destination line ends up
holding one `//`.

`unformatted_ours_shell.svelte` is the authoring under test (the comments written *inside*
the shells); `input.svelte` is where tsv takes it, and `variant_shell.svelte` is where
prettier takes it. Both are stable in both formatters, so the parting is visible only from
that source — `input.svelte` itself is a prettier fixed point.

**tsv**: relocates only when the destination line is clear, and otherwise leaves the run
inside the shell, where the branch's own emitter prints it after the operator:

```
type A = T extends U
	? // c1
		// c2
		V
	: W;
```

**Prettier**: relocates the run's first comment regardless, so `// c1` lands on the
extends-type's line and `// c2` after the `?` — splitting a run the author wrote as one
unit across a syntactic boundary. Before the bound, tsv relocated the whole run onto that
one line, where the two rendered back to back and the second `//` became text of the first
(`T extends U // c1 // c2`): one comment where the author wrote two, irreversibly, the
merged form being a fixed point in both formatters.

Keeping an authored run together, in the region it was written in, is the rule
[consecutive_branch_comment](../consecutive_branch_comment/) and
[extends_question_own_line_line_comment](../extends_question_own_line_line_comment_prettier_divergence/)
already state for the `?`→branch gap's own run. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **A**, **C** — a run of **two** inside the shell, true and false position. The
  divergence: tsv keeps the pair together after the operator, prettier splits it.
- **B**, **D** — **one** comment in the shell with the destination line already taken by a
  comment trailing the anchor. Both formatters decline the relocation here, so these are
  the matching controls — the bound is a bound, not a blanket refusal.
- **E** — the control the licence rests on: a lone comment with a clear destination line
  still relocates, in both formatters.
