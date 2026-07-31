# Member-only chain with a PARENTHESIZED head and interior line comments

The parenthesized-head sibling of
[member_only_interior_line_comment](../member_only_interior_line_comment_prettier_divergence/):
a call-free member chain whose head needs grouping parens (a cast, an `await`,
an arrow) with a line comment in the gap before the member — `(b as T) // c⏎.d`.

- **tsv**: applies the same rule the bare-head chain already gets — the chain
  breaks at the member and the comment stays on the head's line. The head's
  parens stay inline (they fit).
- **prettier**: keeps the comment in the same gap but expands the parens over
  three lines (`const a = (⏎\tb as T⏎) // c⏎.d;`), the outer group breaking on
  the comment rather than the chain.

Both formatters keep the comment where the author wrote it, so the divergence is
about **which group pays for the break**, not about comment position. What makes
the case worth pinning is the second one: without the chain break the comment is
deferred to end-of-line, where the statement's own trailing comment lands too and
the two merge into `.g; // c1 // c2` — the second `//` becomes text of the first
and a comment is lost. That is the same information loss the bare-head fixture's
README describes, reached by the same route.

Reason: Comment relocation. See
[conformance_prettier.md §Comment relocation](../../../../../../../docs/conformance_prettier.md#comment-relocation).
