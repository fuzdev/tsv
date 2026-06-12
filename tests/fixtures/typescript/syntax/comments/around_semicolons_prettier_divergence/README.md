# around_semicolons_prettier_divergence

Comments between the last expression and `;` are preserved in place.

- Input: `const value1 = 1 /* comment2 */;`
- Prettier: `const value1 = 1; /* comment2 */` (moves after semicolon)
- Ours: `const value1 = 1 /* comment2 */;` (preserves before semicolon)

Per comment placement policy, the user's chosen position is preserved.
Both positions are dual-stable in our formatter.

This covers any trailing comment before `;` on a declaration initializer or
ternary branch — including the form a redundant grouping paren strips to
(`const a = (x /* c */)` → `const a = x /* c */;`), which both formatters strip
(see the regular fixture `expressions/parenthesized/stripped_paren_comment`);
only the `;` position then differs.
