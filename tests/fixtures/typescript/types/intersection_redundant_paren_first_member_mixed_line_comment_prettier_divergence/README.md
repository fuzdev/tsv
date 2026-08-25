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
keeps `/* b */ // c` **glued** on one line. tsv holds that form too — a pair the
author glued onto one line keeps it wherever a comment run is emitted
(`docs/comments.md` §Trailing and dangling runs) — so the terminal is **dual-stable**:
a `variant_glued`, pinned by the `prettier_intermediate_to_variant_*` marker
(rule N7b). That is the same shape as the trailing counterpart below; the two
cases must not differ on it, and this seam is the only place they could.

This is the intersection analog of the whole-RHS
[type_alias_rhs_mixed_trailing_comment](../comments/type_alias_rhs_mixed_trailing_comment_prettier_divergence/)
extended through the first-member hoist. The trailing counterpart is
[intersection_redundant_paren_first_member_trailing_line_comment](../intersection_redundant_paren_first_member_trailing_line_comment_prettier_divergence/);
the pure-line counterpart keeps the same trail-on-`=` canonical
([intersection_leading_line_comment](../intersection_leading_line_comment_prettier_divergence/)).

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
