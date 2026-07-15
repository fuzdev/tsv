# Divergence: `export`→`default` keyword-interior comment, decorated class (preserve)

A block comment inside the `export default` keyword when the value is a **decorated class
expression** (`export /* c */ default @dec class {}`). tsv keeps it after `export`; prettier
**relocates** it past `default`.

```ts
// tsv (preserve)          // prettier (relocate past the keyword)
export /* c */ default     export default /* c */
@dec                       @dec
class {}                   class {}
```

The decorated-class path of [default_keyword_comment](../default_keyword_comment_prettier_divergence/) —
a decorator after `default` makes the class an *expression*, so it takes a separate printer path
(the decorators always print on their own line). Same gap, same reason to preserve; this fixture
pins that the path claims the gap too.

See [conformance_prettier.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
