# intersection_leading_line_comment_prettier_divergence

A leading line comment on the first member of an intersection type, written
trailing the `=` (`type C = // leading\n\ta & b;`).

**tsv** keeps the comment trailing the `=`, with the intersection on a
continuation line indented one level. **Prettier** relocates the comment to its
own line after the `=` (`variant_own_line`). Both the input and the own-line
form are stable under tsv (dual-stable). The same pattern applies when the inner
type is a parenthesized union (`(a | b) & c`).

When the comment is authored *inside* a redundant paren shell
(`unformatted_ours_parens`), tsv strips the shell and hoists the comment to the
same trailing-`=` continuation form as the input — indented, so the result is
idempotent. A *doubly* nested redundant paren (`unformatted_ours_double_parens`)
strips and hoists identically — the whole paren shell is scanned, not just its
outermost layer. Prettier's chains from both are pinned by
`audit_signature_<suffix>.txt`.

⚠️ **Which shell strips is the point, and `M3` is where it matters.** At `C` the
shell wraps the first member and that member (`a`) needs no parens of its own, so
nothing survives and the run has to move. At `M3` the member IS a paren-union,
whose pair the position requires — so a shell written around *that member* is the
pair the printer keeps, and the comment stays inside it (see
[intersection_retained_paren_first_member_leading_line_comment](../intersection_retained_paren_first_member_leading_line_comment_prettier_divergence/),
where the extra-layer spellings of that shape live). `M3`'s variants therefore
wrap the **whole value**, which is a shell that really does strip. The rule is the
member-parens one, not the layer count: a comment never changes which parens are
retained, only where it renders once they are.

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(after the `=`) and indents the intersection continuation rather than floating it
to its own line.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
