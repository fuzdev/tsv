# intersection_redundant_paren_first_member_trailing_line_comment_prettier_divergence

A redundant paren shell around the FIRST member of an intersection whose leading
gap holds a **line** comment and whose trailing gap holds a **block** after the
member (`(// c⏎ A /* t */) & B`), and the double-nested form.

**tsv** strips the shell and hangs the run at the same fixed point the bare
authoring settles on — the line comment trails `=`, the trailing block trails the
member inline, and the intersection stays inline on the continuation line:

```
type U = // c
	A /* t */ & B;
```

**Prettier** breaks after `=` and drops the comment onto its own line — the
own-line form (`variant_own_line`), which is dual-stable (tsv keeps it too). On
the paren shell prettier is non-idempotent: its unstable first pass breaks the
intersection (`A /* t */ &⏎ B`) before converging to the own-line variant, so the
shells are `unformatted_ours` + `prettier_intermediate_to_variant`.

The pure-line counterpart (`(// c⏎ A) & B`) keeps the same trail-on-`=` canonical
— see [intersection_leading_line_comment](../intersection_leading_line_comment_prettier_divergence/).
Only the addition of the trailing block makes tsv normalize the paren shell in one
pass where prettier takes two.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
