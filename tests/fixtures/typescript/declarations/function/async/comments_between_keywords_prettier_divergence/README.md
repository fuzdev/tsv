# comments_between_keywords_prettier_divergence

Comments between async and function keywords are preserved in place.

- Input: `async /* comment */ function* F() {}`
- Prettier: `async function* /* comment */ F() {}` (moves to before name)
- Ours: `async /* comment */ function* F() {}` (preserves between keywords)

Per comment placement policy, the user's chosen position is preserved.
Both positions are dual-stable in our formatter.
