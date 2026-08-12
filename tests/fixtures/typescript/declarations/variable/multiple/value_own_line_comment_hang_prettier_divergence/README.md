# value_own_line_comment_hang_prettier_divergence

A **multi-declarator** declaration whose first value gap holds an own-line comment. Both
formatters hang the value — the `=` ends its line and the comment leads the value below it
— and both indent the *sibling* declarators one level. They differ only in how deep the
hung value sits: **tsv gives it that same one level, prettier gives it two.**

The three cases are the three spellings of "own-line comment in this gap", and the point of
the fixture is that they now answer alike:

- `cast` — a JSDoc cast whose comment the author gave a line of its own. The comment is
  **owned** (glued to the cast's `(`, printed from inside the value's doc), so no gap probe
  can see it; the hang comes off `Printer::owned_leading_comment_effect`.
- `block` — an indentable owned block (`/**⏎ * c⏎ */`), the general owned case, same lookup.
- `gap` — a plain `//`, which the gap emits. This one is the **anchor**: it has always
  hung at one level, and it is the form the two owned spellings are made to match.

The cast case is also why the hang is not optional. The cast prints a **hardline** between
its comment and its `(` on exactly this authoring, and a hardline with no hang leaves the
`(` at the declarator-list's own indent — a form the next pass reads as mid-line, hence not
own-line, hence collapses. That authoring had no fixed point at all, which is the same
failure the sibling
[`jsdoc_type_cast_spine_own_line`](../../../../syntax/comments/jsdoc_type_cast_spine_own_line/)
pins for the single-declarator sites.

## Reason

**Design choice** — an indent-only divergence. tsv's hang is the
[§Uniform Forced-Continuation Indent](../../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent):
one level, everywhere a comment splits a construct's head from its tail. The multi-declarator
list already spends that level on its own continuation, and prettier spends a second one to
keep the hung value distinguishable from the next declarator; tsv does not, because the rule
is the construct's, not the list's, and re-deriving it per container is what makes indents
drift. The `gap` case shows the level was tsv's answer here before the owned ones joined it.

The comment itself does not move in either formatter, so this carries no
[§Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
difference — only the depth.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
