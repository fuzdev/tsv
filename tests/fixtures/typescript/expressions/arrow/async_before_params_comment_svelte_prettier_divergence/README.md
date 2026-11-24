# async_before_params_comment_svelte_prettier_divergence

A comment between an async arrow's `async` keyword and its parameters is kept in
the gap the author wrote it in. Prettier relocates it, and where it lands depends
on the shape of the arrow.

- Input: `async /* c */ () => {}` — Prettier: `async () => /* c */ {}` (into the body)
- Input: `async /* c */ (x) => x` — Prettier: `async (/* c */ x) => x` (into the parameter list)
- Input: `async /* c */ <T extends A>(x: T) => x` — Prettier: unchanged (in place)
- Ours: all three preserve the authored position

Prettier answers this one gap three ways, keyed on what follows `async`, so its
placement is no oracle here: the same comment reads as being about the body, about
the first parameter, or about nothing in particular depending on whether the arrow
has parameters or type parameters. A run keeps its order and its gap under tsv;
Prettier carries the whole run into the body.

Only a **single-line block** comment can occupy this gap at all — a `//`, or a
block comment with an interior newline, puts a line terminator between `async` and
the parameters, which `AsyncArrowFunction : async [no LineTerminator here]
ArrowFormalParameters` (ecma262) forbids. Those rejections are pinned by
`../async_line_break/`.

Not preserving drops the comment: the gap has no other emitter.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.

## Svelte divergence (parser)

The generic case `async /* c */ <T extends A>(x: T) => x` is also a parse divergence.
tsv's wire gives `ArrowFunctionExpression` one key order — acorn-typescript's common
route's, with `typeParameters` last — where acorn-typescript's generic async route
(`tsTryParseGenericAsyncArrowFunction`) writes `typeParameters` first. Svelte's comment
walk visits keys in order, so canonical gives `c` to the type parameter declaration's
`leadingComments` and tsv gives it to the first parameter `x`'s. The comment is kept,
on the same arrow head, and in the root `comments` array of both parsers; the other
three cases attach the same in both. See
[conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences, and `../async_generic/params_comment_return_type_svelte_divergence/` for
the route with a return type.
