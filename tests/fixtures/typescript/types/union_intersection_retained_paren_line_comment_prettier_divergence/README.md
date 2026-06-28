# union_intersection_retained_paren_line_comment_prettier_divergence

Trailing **line** comment inside a **retained** parenthesized union member — a
`(x | y)` whose parens are kept because it nests in an outer union, with a line
comment trailing the last inner member (`(a | b // c) | c`, `a | (b | c // c) | d`).

Both formatters keep the comment **inside** the parens and break the parens onto
their own lines because a line comment must end its line. They differ only on the
inner union layout: **Prettier** keeps the inner union exploded, one member per
line; **tsv** applies its union-fit layout — a union broken from its parent
re-collapses onto one line when it fits — so the inner union stays inline with the
comment trailing the last member.

The comment is preserved in place in both forms, so this is a pure layout
difference (inline vs exploded inner union), not comment relocation. The
block-comment sibling `union_intersection_retained_paren_comment` keeps the member
fully inline because a block comment can stay inline (`(b | c /* c */)`); a line
comment cannot, so it forces the expanded parens.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
