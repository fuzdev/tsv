# template_literal_interp_trailing_comment_prettier_divergence

A comment authored **trailing** a template-literal type's `${` stays trailing it, with the
type dropped to the next line and `}` on its own line — a `//` can't swallow the type, and a
multiline block's authored break isn't reflowed. Covers both comment kinds (line, multiline
block) plus the width case (`V`).

- **tsv** keeps the comment on the `${` line (`` `a${// c⏎⇥B⏎}` ``).
- **Prettier** expands the interpolation, moving the comment down onto its own line
  (`` `a${⏎// c⏎B⏎}` ``).

`}` lands on its own line here for the same reason it does under a width-driven break: once
the interpolation is broken, it takes the broken shape. The comment is the *only* break
mechanism in play — `V` shows a too-wide type simply breaking *inside* the already-broken
interpolation, rather than a second break firing on top of it.

Prettier is stable on its own output here, so this is a pure comment-position divergence:
per [Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment on the line the author put it on.

## Why this is authoring-dependent, and deliberately so

The **own-line** authoring (`` `a${⏎// c⏎B}` ``) is *not* pinned here — tsv expands it and
matches prettier ([template_literal_interp_own_line_comment](../template_literal_interp_own_line_comment/)).
The two authorings reach two different fixed points **on purpose**, because the authored
break carries the signal and each form is stable, so nothing is lost either way.

This is the same distinction tsv's type-alias `=` gap already draws — `type A = /* c */ B`
keeps the comment on the `=` line, `type A =⏎/* c */⏎B` keeps it on its own — and it is what
lets tsv preserve *both* authorings. A single canonical form cannot: forcing the expansion
moves a trailing-authored comment down (prettier's choice), and forcing the flush form pulls
an own-line-authored one up. Only the trailing authoring diverges from prettier; the own-line
one matches, so the divergence is the minimum needed to preserve the author's placement.

The `=` keeps the backtick on its line in both (`value_owns_its_comment_break`), since the
template already breaks itself.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
