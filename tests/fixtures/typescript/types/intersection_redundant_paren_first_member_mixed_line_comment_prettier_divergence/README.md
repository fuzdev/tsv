# intersection_redundant_paren_first_member_mixed_line_comment_prettier_divergence

A redundant paren shell around the FIRST member of an intersection whose leading
gap holds a **block before a line comment** (mixed, `(/* b */ // c⏎ A) & B`), and
the double-nested form.

**tsv** strips the shell and hangs the run at the same fixed point the bare
authoring settles on — the block trails `=` inline, the line comment forces the
value onto its own line, and the intersection stays inline on the continuation:

```
type T = /* b */
	// c
	A & B;
```

**Prettier** breaks after `=` and drops the whole run onto its own line(s)
(`output_prettier`). On the paren shell prettier is non-idempotent: its unstable
first pass breaks the intersection (`A &⏎ B`) before converging to a form that
keeps `/* b */ // c` **glued** on one line — a form tsv un-glues to its own
`output_prettier`, so it is a `divergent_variant`, pinned by the
`prettier_intermediate_to_divergent_variant_*` marker (rule N7c).

This is the intersection analog of the whole-RHS
[type_alias_rhs_mixed_trailing_comment](../comments/type_alias_rhs_mixed_trailing_comment_prettier_divergence/)
(bug188), extended through the first-member hoist. The trailing counterpart is
[intersection_redundant_paren_first_member_trailing_line_comment](../intersection_redundant_paren_first_member_trailing_line_comment_prettier_divergence/);
the pure-line counterpart keeps the same trail-on-`=` canonical
([intersection_leading_line_comment](../intersection_leading_line_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
