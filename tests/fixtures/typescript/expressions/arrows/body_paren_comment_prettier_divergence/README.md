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
- With a leading and a trailing comment, the leading comment hugs `=>` (both
  formatters keep it there) while the trailing comment keeps the parens.
- For an object-literal body the parens are **required** (object/block
  disambiguation), not redundant grouping parens. tsv keeps the comment inside
  (`({ k: 1 } /* c */)`); prettier moves it outside the required paren
  (`({ k: 1 }) /* c */`), changing its association from the object to the whole
  expression.

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Arrow body stripped parens) and §Comment Position
Philosophy.
