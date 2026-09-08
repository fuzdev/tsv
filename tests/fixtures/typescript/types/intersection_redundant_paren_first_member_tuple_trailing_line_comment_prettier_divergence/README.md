# intersection_redundant_paren_first_member_tuple_trailing_line_comment_prettier_divergence

The tuple-element analog of
[intersection_redundant_paren_first_member_trailing_line_comment](../intersection_redundant_paren_first_member_trailing_line_comment_prettier_divergence/):
a redundant paren shell around the FIRST member of an intersection whose leading
gap holds a **line** comment and whose trailing gap holds a **block** after the
member (`(// c⏎ A /* t */) & B`), now inside a **tuple element** rather than a
type-alias RHS. The double-nested form behaves the same.

**tsv** strips the shell and normalizes to the tuple element's own fixed point —
the line comment keeping the `[` line it was written on (the opening-delimiter rule the
tuple's gap answers for every route into it, the stripped shell's run included —
[open_bracket_comment](../tuple/open_bracket_comment_prettier_divergence/)), the trailing
block inline after the member, and the intersection inline on the continuation:

```
type T = [ // c
	A /* t */ & B
];
```

Prettier un-glues the `[` line and settles on the own-line form (`output_prettier`), which
it reaches from the shell authorings non-idempotently — its unstable first pass breaks the
intersection (`A /* t */ &⏎ B`) before converging — so those chains are pinned by
`audit_signature_*.txt`. The type-alias RHS sibling differs in kind: there prettier breaks
after `=` and settles on a distinct own-line `variant`.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
