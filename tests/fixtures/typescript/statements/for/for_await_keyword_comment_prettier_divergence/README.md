# for_await_keyword_comment_prettier_divergence

Comments between `await` and `(` in `for await` are preserved in place.

- Input: `for await /* c */ (const x of y) {}`
- Prettier: `for await (/* c */ const x of y) {}` (absorbs into parens)
- Ours: `for await /* c */ (const x of y) {}` (preserves between keyword and paren)

Per comment placement policy, the user's chosen position is preserved.
Both positions are dual-stable in our formatter.
