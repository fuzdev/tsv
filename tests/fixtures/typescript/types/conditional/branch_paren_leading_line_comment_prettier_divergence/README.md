# branch_paren_leading_line_comment_prettier_divergence

A redundant paren shell around a conditional type's **branch** whose leading gap holds a
line comment the author **glued to the `(`** (`? (// c⏎V)`). The shell strips, and the
comment stays in the `?` / `:` gap it was written in, trailing the operator — exactly where
the paren-free authoring `? // c⏎V` puts it, in both formatters. Prettier instead
**relocates** it across the operator to trail the node before it: the extends-type for the
`?` arm, the true branch for the `:` arm.

`unformatted_ours_shell.svelte` is the authoring under test (the comments written *inside*
the shells; `unformatted_ours_double_shell` and `unformatted_ours_compact` spell the same
thing with a double pair and flush); `input.svelte` is where tsv takes it, and
`variant_shell.svelte` is where prettier takes it. Both are stable in both formatters, so
the parting is visible only from that source — `input.svelte` itself is a prettier fixed
point.

**tsv**:

```
type A = T extends U
	? // c
		V
	: W;
```

**Prettier**: `type A = T extends U // c⏎\t? V⏎\t: W;` — the comment re-bound from the branch
it led to the extends-type. On a run of two it relocates only the first, splitting a run the
author wrote as one unit across the operator (`T extends U // c1⏎\t? // c2⏎\t\tV`).

## Reason

A comment between an operator and its operand stays there — principle 1 of
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
whose own example is this gap's paren-free spelling
([comment_after_colon](../comment_after_colon_prettier_divergence/)). The shell adds nothing
to the question: a leading comment never changes which parens are retained, only where it
renders once they are, and a shell that strips leaves its run in the enclosing gap (the
paren corollary in the same section). The value-level conditional answers the identical
authoring the same way (`cond ? (// c⏎b) : c` → `? // c⏎\t\tb`), and so does this gap's
own-line spelling `? (⏎// c⏎V)`
([branch_own_line_line_comment](../branch_own_line_line_comment_prettier_divergence/)) — so
the type-level glued spelling was the one cell where the shell alone moved a comment across
the operator. Keeping it in the gap is also what keeps a run of two together, one comment per
line ([consecutive_branch_comment](../consecutive_branch_comment/)), where relocating the
whole run welded the pair (`T extends U // c1 // c2`, the second `//` becoming text of the
first) and relocating half of it splits them.

The `extends`-type's shell keeps its own trail-on-inner canonical
([extends_paren_leading_line_comment](../extends_paren_leading_line_comment/)): that gap has
no operator between the comment and its operand, only the `extends` keyword the comment
already follows.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **A / B** — one comment in the true and the false branch's shell.
- **C** — a run of two: tsv keeps the pair together after the operator, prettier splits it.
- **D / E** — a comment already trailing the anchor: both formatters keep the shell's comment
  in the gap (there prettier has nowhere lossless to put it), so these are the matching
  controls.
- **F / G** — a shell around a nested conditional, in both positions.
- **H** — a nested-conditional shell whose trailing gap holds a `//` too. In the true
  position the shell strips either way (the outer `:` flushes the trailing comment —
  [branch_paren_trailing_line_comment](../branch_paren_trailing_line_comment_prettier_divergence/)),
  so the leading comment takes the gap exactly as in F. The claim gate and the branch builder
  read one retention predicate (`Printer::branch_shell_retains`): with the gate reading the
  general shell rule instead, the run was left to the shell's own emitter at the shell's
  indent, and pass 2 — the shell gone — hung the branch below it.
