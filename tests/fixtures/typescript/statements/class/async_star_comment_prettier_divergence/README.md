# async_star_comment_prettier_divergence

A comment between the `async` keyword and the generator `*` of a method is
preserved at the author's position. Prettier relocates it to after the `*`
(before the name).

- Input: `async /* c */ *m() {}`
- Prettier: `async */* c */ m() {}` (moves the comment after `*`)
- Ours: `async /* c */ *m() {}` (preserves between `async` and `*`)

A `*` inside the comment (`/* a * b */`) is not mistaken for the generator
star — the delimiter scan skips comment contents. The after-`*` position
(`*/* comment */ m()`) is preserved identically by both formatters — see
`../generator_method_comment/`.

Per comment placement policy, the user's chosen position is preserved when
prettier moves comments to different positions.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
