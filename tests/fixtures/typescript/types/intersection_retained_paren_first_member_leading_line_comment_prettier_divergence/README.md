# intersection_retained_paren_first_member_leading_line_comment_prettier_divergence

Leading **line** comment inside a **retained** parenthesized union that is the **first**
member of an intersection (`(// c\n A | B) & C`). The paren is the union's own required
pair — it survives into the output — so the comment sits inside a shell the formatter keeps.

**Prettier** hoists the comment out of that pair, and does it in two steps: its first pass
lands it after the enclosing shell's `(` (`| (// c\n (A | B) & C)`, `output_prettier.svelte`)
and its second hoists it out again in front of the pair (`| // c\n ((A | B) & C)`), which is
where it settles — pinned by `audit_signature.txt`. **tsv** keeps the comment where the user
wrote it, inside the parens leading the inner union. Because a line comment must end its
line, the pair expands (`)` on its own line) while the inner union stays inline when it fits.

Per Comment Position Philosophy: the comment is inside the parenthesized member, so tsv
associates it with that member rather than hoisting it out — the same answer a **later**
intersection member already gives (`A & (// c\n B | C)`), and the same answer the union
family gives at every member, first included, in
[union_intersection_retained_paren_leading_line_comment](../union_intersection_retained_paren_leading_line_comment_prettier_divergence/).
Hoisting was also **non-idempotent**: it relocated the comment into the enclosing shell's
`(`→member gap, where tsv's next pass renders it through that gap's own emitter instead
(one normalizing space after the `(`, a different break) — so the first member disagreed
with its own reparse. Keeping it inside is a fixed point in one pass.

`Plain` is the base spelling: the alias value *is* the intersection, so the union's required
pair is the only shell in play and nothing encloses it. `FirstTrailingObject` is the same
shell whose last intersection member is an object literal, which supplies its own aligned
layout; the comment is kept inside the first member's parens just the same. `FirstOwnLine`
writes the comment on its own line inside the pair and keeps that line — which line the
comment takes is the author's, per the opening-delimiter rule. `TupleElement` is the
**own-line** context: the caller already places the element on its own indented line, so the
member keeps the comment inside at the element's indent with no extra continuation level.

The boundary is which pair the comment sits in, and it is asked of the paren's **direct**
child. A **redundant** outer layer (`(// c\n (A | B)) & C`, `((// c\n A | B)) & C`) is
stripped, so the pair holding the run does not survive and the comment must hoist to the
value seam instead — that case keeps its existing answer in
[intersection_leading_line_comment](../intersection_leading_line_comment_prettier_divergence/).

`TransparentShell` is that boundary reached from the other side — the control for the rule's
one exception. A **single-member** union (the leading-`|` spelling, `(// c\n | A) & B`) parses
as a union but needs no pair, so its shell is redundant and the enclosing value seam strips it
and claims the run. The keep-inside answer must stand down there: given at a shell that does
not survive, it prints the comment **twice** — once from the seam, once from the member. tsv
hoists it to the value seam and prints it once, keeping the intersection inline where prettier
breaks it (that hang shape is the divergence cataloged in
[intersection_leading_line_comment](../intersection_leading_line_comment_prettier_divergence/);
the case earns its place here as the boundary's negative side).
`TransparentShellOperatorGap` is that same shell reached by the layout's **other** trigger — a
second comment in the member gap (`(// c\n | A) // x\n & B`) routes the intersection
comment-aware on its own, without the router's claim question ever being asked — so the
stand-down has to live at the member emitter, not only at the router. Both spellings print
`// c` exactly once.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
