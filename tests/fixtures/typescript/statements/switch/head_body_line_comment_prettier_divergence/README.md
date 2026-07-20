# head_body_line_comment_prettier_divergence

A comment in the `switch (…)`→`{` gap.

tsv: keeps the comment where the author wrote it, dropping `{` to the next line after a `//`
Prettier: absorbs it into the switch body, or into the discriminant parens

## Reason

⚠️ **This gap previously produced unreparseable output.** The run was emitted inline and then
a bare `" {"` appended, so a line comment **swallowed the opening brace**:

```js
switch (a) // c1 {     ← the `{` is inside the comment; `case 1:` then fails to parse
	case 1:
```

That is content corruption, not a layout preference, which is why tsv holds a position here at
all. The gap now routes through the shared header→body emitter used by `if` / `while` / `for` /
`do` / `try`, so a `//` forces the `{` onto the next line and can no longer absorb it.

Prettier is no oracle: it relocates the comment away entirely — into the block body for an
own-line comment, and into the **discriminant parens** for one trailing `)`
(`switch (⏎↹a // c2⏎) {`), which re-binds it to the discriminant. Per
[Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the author's placement.

## Cases

Own-line line comment, trailing-`)` line comment, a blank between two own-line comments
(preserved), and both block-comment authorings. A block comment keeps its authored line like
every other kind: authored trailing `)` it stays there with `{` hugged (the one shape where
both formatters keep the brace hugged), authored on its own line it keeps that line and `{`
drops below — where prettier absorbs it into the body, same as it does a `//`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
