# intersection_redundant_paren_first_member_tuple_mixed_line_comment_prettier_divergence

The tuple-element analog of
[intersection_redundant_paren_first_member_mixed_line_comment](../intersection_redundant_paren_first_member_mixed_line_comment_prettier_divergence/):
a redundant paren shell around the FIRST member of an intersection whose leading
gap holds a **block before a line comment** (mixed, `(/* b */ // c⏎ A) & B`), but
now the intersection sits inside a **tuple element** (`[(…) & B]`) rather than a
type-alias RHS. The double-nested form behaves the same.

**tsv** strips the shell and hangs the run at the tuple element's own fixed point
— the block and line comment each on their own line inside `[`, and the
intersection inline on the continuation:

```
type T = [
	/* b */
	// c
	A & B
];
```

Both this canonical and a **glued** form (`/* b */ // c` on one line) are stable
under both formatters, so the glued form is a `variant`. On the paren shell
prettier is non-idempotent: its unstable first pass breaks the intersection
(`A &⏎ B`) before converging to the glued variant, so the shells are
`unformatted_ours` + `prettier_intermediate_to_variant`.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
