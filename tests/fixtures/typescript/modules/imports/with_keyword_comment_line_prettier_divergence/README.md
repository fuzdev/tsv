# with_keyword_comment_line_prettier_divergence

A **line comment between an import's source and its `with` attributes keyword**
makes prettier's `typescript` parser throw:

```
import b from './b' // c2
with {type: 'json'};
```

```
'(' expected.
```

The line comment forces the `with` onto the next line, and typescript-estree
fails to parse the import-attributes clause. Our parser accepts it (matching
Svelte / acorn-typescript) and our formatter keeps the comment where the user
placed it, so prettier cannot serve as a formatting oracle here — there is no
`output_prettier.*` to record. `prettier_rejects.txt` pins the error message;
rule F6 live-verifies that prettier still rejects the input with that message.

This is a **prettier-core / typescript-estree** bug — it reproduces in plain
prettier with `parser: 'typescript'` and zero Svelte, and is fine under
`babel-ts`. The *block*-comment variant (`/* c1 */`) parses fine; only the
line-comment-before-`with` form breaks. The block-comment forms (source→`with`
and `with`→`{`, plus the line comment *after* `with`) all format and live in
the sibling `with_keyword_comment_prettier_divergence` fixture.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Prettier rejects valid input.
