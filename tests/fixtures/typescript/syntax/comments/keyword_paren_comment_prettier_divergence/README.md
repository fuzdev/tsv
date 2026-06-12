# keyword_paren_comment_prettier_divergence

Comments between a keyword and its `(` are preserved in place.

- Input: `if /* comment */ (a) {}`
- Prettier: `if (/* comment */ a) {}` (absorbs into parens)
- Ours: `if /* comment */ (a) {}` (preserves between keyword and paren)

Applies to `if`, `while`, `for`, `switch`, `catch`, `do...while`.
Per comment placement policy, the user's chosen position is preserved.
Both positions are dual-stable in our formatter.
