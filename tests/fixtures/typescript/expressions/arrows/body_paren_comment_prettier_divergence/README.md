# Arrow body paren comment divergence

When grouping parens around an arrow body expression carry a trailing comment
(`() => (x /* c */)`), tsv preserves the parens to keep the comment in its
original position — the same approach as unary expressions. Prettier strips the
parens and relocates the comment to trail the body, and at its fixed point floats
a same-line line comment past the body's `;`, detaching it from the body
entirely. That past-`;` relocation, not just the paren strip, is the strongest
reason for the divergence: tsv keeps every comment attached to the body where the
author wrote it. The fixture covers a simple body, a call argument, curried
arrows, and own-line and stacked line comments.

A few cases go further:

- Stacked line comments each stay on their own line inside the parens; a line
  comment forces a break before any following comment regardless of kind, so no
  comment is folded into another's text.
- A same-line comment group the author wrote together (a block then a line
  comment, `/* i1 */ // i2`) stays together on one line after a body break.
- With a leading and a trailing comment, the leading comment hugs `=>` while the
  trailing comment keeps the parens. Both formatters keep the leading comment there
  over a plain body; over a **ternary** body prettier prints its own layout paren and
  moves the leading comment inside it (`() => (/* lead */ cond ? a : b /* trail */)`),
  while tsv keeps it outside — the parens tsv prints are the authored ones it retained
  for the trailing comment, so outside them is a position the author chose. Drop the
  trailing comment and the authored parens are stripped, leaving only the ternary's
  layout paren, which vanishes as soon as the body breaks; there tsv converges with
  prettier and puts the comment inside
  ([arrow/body_stripped_paren_comment_long](../../arrow/body_stripped_paren_comment_long/)).
- For an object-literal body the parens are **required** (object/block
  disambiguation), not redundant grouping parens. tsv keeps the comment inside
  (`({ k: 1 } /* c */)`); prettier moves it outside the required paren
  (`({ k: 1 }) /* c */`), changing its association from the object to the whole
  expression.

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Arrow body stripped parens) and §Comment Position
Philosophy.
