# async_star_comment_prettier_divergence

A comment between the `async` keyword and the generator `*` of a method is
preserved at the author's position. Prettier relocates it to after the `*`
(before the name).

- Input: `async /* c */ *m() {}`
- Prettier: `async */* c */ m() {}` (moves the comment after `*`)
- Ours: `async /* c */ *m() {}` (preserves between `async` and `*`)

A **line** comment in the same gap (`static // c`) is preserved in place too, and
forces the break it needs: the `//` ends its line, and the `*`, the key and the
method body follow on the next one at the member's indent. Emitting the gap
inline swallows all three (`static // c *line() {}` does not reparse). Prettier
relocates past the `*` and breaks in its own position (`static *// c⏎line()`).
Only a `static`-style modifier reaches this gap with a line comment: in a class
body a line terminator after `async` ends the member by ASI (`async;` then a
separate generator), so that authoring is a different program.

A `*` inside the comment (`/* a * b */`) is not mistaken for the generator
star — the delimiter scan skips comment contents. The after-`*` position
(`*/* comment */ m()`) is preserved identically by both formatters — see
`../generator_method_comment/`.

Per comment placement policy, the user's chosen position is preserved when
prettier moves comments to different positions.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
