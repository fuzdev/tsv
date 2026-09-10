# body_terminator_glued_comment_run_prettier_divergence

A brace-less `do` body whose pre-`;` gap holds a **glued** run — two block comments the
author wrote on one line, and a block with a `//` behind it. The sibling
[body_terminator_comment_run](../body_terminator_comment_run_prettier_divergence/) covers
the own-line run at this seam; this fixture covers the glued one, the cell that seam's
rule ("each comment on its own line, in authored order") never spelled.

**tsv**: keeps each comment on its own line, in authored order, at the `do`'s level —
the same shape the gap gives once the `;` sits ahead of the run, so one document has one
answer however the author placed the terminator:

```
do expr1;
/* c1 */
/* c2 */
while (a);
```

**prettier**: relocates the whole run INTO the `while` condition's parens, carrying the
author's gluing with it (`while (/* c1 */ /* c2 */ a)`). The `;` and the `while` head are
structure between the comments and the condition; tsv does not carry a comment across
them.

An authored blank between two remarks survives the split — each deferred member carries
its own break and its own blank — and the CONTROL case is the boundary: a run the author
glued to the body's OWN line is a *trailing* run, keeps that line in both formatters, and
is untouched by this rule, which reaches only what starts below the content's line.

The clause tail is one of the two heads that stay OPEN to a continuing construct (the
other is `if`+`else`), so its terminator gap defers through `line_suffix` rather than
being handed to the enclosing statement list. Preserving the author's glue *there* while
the ejected seam breaks the run gave the same document two shapes — the pre-`;` authoring
printed `/* c1 */ /* c2 */` on one line and then split it on a second pass. That authoring
is the `unformatted_ours_pre_terminator` form; `variant_pre_terminator` is prettier's own
landing from it, which tsv keeps stable.

Reason: comment position and order preserved over prettier's relocation into the
condition, and one shape per document over the author's terminator placement. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
