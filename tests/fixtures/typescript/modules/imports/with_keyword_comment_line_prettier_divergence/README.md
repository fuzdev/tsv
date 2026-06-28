# with_keyword_comment_line_prettier_divergence

A **line comment between an import's source and its `with` attributes keyword**:

```
import b from './b' // c2
	with { type: 'json' };
```

Under prettier 3.8 this threw in prettier's `typescript` parser (`'(' expected.`,
a typescript-estree bug); prettier 3.9 **accepts** it. It collapses `with {…}`
back onto the source line and floats the line comment past the `;`
(`import b from './b' with { type: 'json' }; // c2`). tsv keeps the comment
between the source and `with`, forcing `with {…}` onto the next line and
indenting the continuation one level.

```ts
// prettier 3.9 (comment floated past `;`)        // tsv (comment kept before `with`)
import b from './b' with { type: 'json' }; // c2   import b from './b' // c2
                                                   	with { type: 'json' };
```

The block-comment forms (source→`with` and `with`→`{`, plus the line comment
*after* `with`) all format and live in the sibling
`with_keyword_comment_prettier_divergence` fixture.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
