# Conditional `?` / `:`→branch gap, own-line comment run

The **own-line** authoring of a comment between a conditional's `?` or `:` and the branch it
introduces. Authored on the operator's line (`? // c⏎\t\tb`) the comment trails the operator
in both formatters ([branch_comment_paren](../branch_comment_paren/)); here prettier
**relocates** — it pulls the first comment up onto the operator's line — while tsv keeps the
line the author gave it, leaves the operator alone on its line and hangs the branch one
level in below the run.

```
const a = cond          const a = cond
→?                      →? // c1
→→// c1                 →→b
→→b                     →: c;
→: c;
```

## Reason

A comment in this gap **leads the branch**, and own-line-ness is authoring signal for a
leading position — the corollary in
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
§Comment Position Philosophy. A branch gap is a value gap, so it takes the keyword→value
family's answer: a same-line comment trails the operator, an own-line comment keeps its own
line. The gap already answered this way for an own-line `// prettier-ignore`
([branch_prettier_ignore_head](../branch_prettier_ignore_head_prettier_divergence/)), so a
directive and a plain comment keep the same placement, and the conditional type's branches
take the same answer
([branch_own_line_line_comment](../../../types/conditional/branch_own_line_line_comment_prettier_divergence/)).

What the cases pin:

- **c1/c2** — the `?`→consequent and `:`→alternate gaps.
- **c3/c4** — a branch's clarity parens (`??`, `as`) survive the hang, and so does a comment
  trailing the alternate.
- **c5/c6** — a run keeps one comment per line, in order; prettier pulls only the first up.
- **c7/c8** — a **block ahead of the line comment** keeps its own line too, where prettier
  pulls the block up alone.
- **c9** — a branch that breaks on its own hangs the same way.
- **c10** — a chained conditional's gap, one level deeper.
- **c11** — the control: a comment authored **on** the operator's line trails it in both
  formatters. Both authorings are stable under tsv — neither moves the comment to the
  other's line.

`unformatted_ours_compact` authors every case flush and unspaced;
`unformatted_ours_paren_shell` puts the `c1`, `c3` and `c4` runs inside a redundant paren
shell around the branch, which strips to the same fixed point.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
