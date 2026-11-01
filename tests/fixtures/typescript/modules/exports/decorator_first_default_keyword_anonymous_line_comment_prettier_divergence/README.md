# Divergence: `default`→`class` line comment, decorators-first anonymous class (preserve)

A `//` in the `default`→`class` gap when the **decorators precede `export`** and the class has
**no name**. tsv keeps it after `default` and indents the `class` continuation one level (the
uniform forced-continuation indent); prettier **relocates** it onto the decorator line.

```ts
// tsv (preserve)            // prettier (relocate onto the decorator)
@dec                         @dec // c
export default // c          export default class extends B {}
	class extends B {}
```

The block-comment form of the same gaps is
[decorator_first_default_keyword_anonymous_comment](../decorator_first_default_keyword_anonymous_comment_prettier_divergence/).
A heritage clause follows the bare `class` keyword, so the comment must print once, ahead of
`class`, and never again before `extends`.

See [conformance_prettier_ts_comments.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier_ts_comments.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
