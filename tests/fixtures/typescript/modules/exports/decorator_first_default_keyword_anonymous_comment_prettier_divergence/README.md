# Divergence: `export default` keyword comments, decorators-first anonymous class (preserve)

The anonymous-class twin of
[decorator_first_default_keyword_comment](../decorator_first_default_keyword_comment_prettier_divergence/):
block comments inside the `export default` keyword when the **decorators precede `export`** and
the class has **no name**. tsv keeps each where the author wrote it; prettier **relocates both** up
onto the decorator line.

```ts
// tsv (preserve)                              // prettier (relocate onto the decorator)
@dec                                           @dec /* c1 */ /* c2 */
export /* c1 */ default /* c2 */ class {}      export default class {}
```

Without a name, nothing stands between the `class` keyword and the body, so the `default`→`class`
gap is the only position `c2` was authored in and each comment prints exactly once, in its own
gap. The relocation also collapses which side of `default` each comment led, as in the named
twin. The line-comment form of the `default`→`class` gap is
[decorator_first_default_keyword_anonymous_line_comment](../decorator_first_default_keyword_anonymous_line_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier_ts_comments.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
