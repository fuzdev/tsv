# intersection_redundant_paren_first_member_tuple_mixed_line_comment_prettier_divergence

The tuple-element analog of
[intersection_redundant_paren_first_member_mixed_line_comment](../intersection_redundant_paren_first_member_mixed_line_comment_prettier_divergence/):
a redundant paren shell around the FIRST member of an intersection whose leading
gap holds a **block before a line comment** (mixed, `(/* b */ // c⏎ A) & B`), but
now the intersection sits inside a **tuple element** (`[(…) & B]`) rather than a
type-alias RHS. The double-nested form behaves the same.

**tsv** strips the shell and hangs the run at the tuple element's own fixed point —
the pair keeping the one line the author wrote it on, and the intersection inline on
the continuation:

```
type T = [
	/* b */ // c
	A & B
];
```

**Glue is the author's, not the seam's.** The unglued form is stable too — both
formatters keep it — so it is a `variant`, reached from the unglued authoring of the
same pair. The two authorings keep two fixed points because the tuple element's gap
preserves each; what it must NOT do is answer the shell authoring differently from
the bare authoring of the same glue, which is what
[head_paren_shell_member_gap_line_comment](../head_paren_shell_member_gap_line_comment_prettier_divergence/)
made the element gap own. The type-alias RHS sibling above reads differently on
purpose: its keyword→value emitter trails the first comment on the `=` line
unconditionally, so the pair cannot stay together there.

On the paren shell prettier is non-idempotent: its unstable first pass breaks the
intersection (`A &⏎ B`) before converging to this input, so the shells are
`unformatted_ours` + `prettier_intermediate`.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
