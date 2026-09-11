# Conditional type `?` / `:`→branch gap, own-line comment run

The **own-line** authoring of a comment between a conditional type's `?` or `:` and the
branch it introduces. Authored on the operator's line (`: // c⏎\t\tbar`) the comment trails
the operator in both formatters
([comment_after_colon](../comment_after_colon_prettier_divergence/)); here prettier
**relocates** — it pulls the first comment up onto the operator's line — while tsv keeps the
line the author gave it, leaves the operator alone on its line and hangs the branch one
level in below the run.

```
type A = B extends C    type A = B extends C
→?                      →? // c1
→→// c1                 →→D
→→D                     →: E;
→: E;
```

## Reason

The value-level conditional's rule, at the type-level twin — see
[branch_own_line_line_comment](../../../expressions/ternary/branch_own_line_line_comment_prettier_divergence/)
for the argument. A comment in a branch gap **leads the branch**, and own-line-ness is
authoring signal for a leading position (the corollary in
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
§Comment Position Philosophy), so a same-line comment trails the operator and an own-line
comment keeps its own line.

What the cases pin:

- **c1/c2** — the `?`→true-branch and `:`→false-branch gaps.
- **c3/c4** — a run keeps one comment per line, in order; prettier pulls only the first up.
- **c5/c6** — a nested conditional's check type and an intersection head hang as a simple
  type does.
- **c7** — a chained conditional's gap, one level deeper.
- **c8** — the control: a comment authored **on** the operator's line trails it in both
  formatters. Both authorings are stable under tsv.

`unformatted_ours_compact` authors every case flush and unspaced;
`unformatted_ours_paren_shell` puts the `c5` and `c6` runs inside a redundant paren shell
around the branch's leading edge, which strips to the same fixed point (prettier's chain
from it is pinned by `audit_signature_paren_shell.txt`).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
