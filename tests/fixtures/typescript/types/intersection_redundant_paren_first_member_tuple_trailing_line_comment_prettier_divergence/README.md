# intersection_redundant_paren_first_member_tuple_trailing_line_comment_prettier_divergence

The tuple-element analog of
[intersection_redundant_paren_first_member_trailing_line_comment](../intersection_redundant_paren_first_member_trailing_line_comment_prettier_divergence/):
a redundant paren shell around the FIRST member of an intersection whose leading
gap holds a **line** comment and whose trailing gap holds a **block** after the
member (`(// c⏎ A /* t */) & B`), now inside a **tuple element** rather than a
type-alias RHS. The double-nested form behaves the same.

**tsv** strips the shell and normalizes to the tuple element's own fixed point —
the line comment on its own line inside `[`, the trailing block inline after the
member, and the intersection inline on the continuation:

```
type T = [
	// c
	A /* t */ & B
];
```

Unlike the type-alias RHS sibling — where prettier breaks after `=` and settles
on a distinct own-line `variant` — the tuple element is on its own line for both
formatters, so prettier's stable form **is** this input. Prettier still reaches it
non-idempotently: its unstable first pass breaks the intersection
(`A /* t */ &⏎ B`) before converging back to the input, so the shells are
`unformatted_ours` + `prettier_intermediate` (converges to input).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
