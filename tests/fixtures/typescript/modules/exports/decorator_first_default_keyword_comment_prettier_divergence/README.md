# Divergence: `export default` keyword comments, decorators-first class (preserve)

Block comments inside the `export default` keyword when the **decorators precede `export`**
(`@dec⏎export /* c1 */ default /* c2 */ class A {}`). tsv keeps each where the author wrote it;
prettier **relocates both** up onto the decorator line.

```ts
// tsv (preserve)                              // prettier (relocate onto the decorator)
@dec                                           @dec /* c1 */ /* c2 */
export /* c1 */ default /* c2 */ class A {}    export default class A {}
```

Decorators before `export` keep the class a *declaration*, so it takes a third printer path —
distinct from [default_keyword_comment](../default_keyword_comment_prettier_divergence/) (the
general path) and from
[default_keyword_decorator_comment](../default_keyword_decorator_comment_prettier_divergence/),
where a decorator *after* `default` makes the class an *expression*. All three share the gap and
the reason to preserve; this fixture pins the declaration path.

Two gaps, both pinned: `export`→`default` (the keyword interior) and `default`→`class` (the
keyword→value gap). The path emitted the keyword as one fixed text, so it scanned neither and
dropped both.

Relocation here also **collapses a distinction**: `c1` and `c2` are authored on opposite sides of
`default` and land side by side on the decorator line, so which one led `default` is no longer
recoverable. That is the same association loss catalogued for `await`/`using` and
`export as namespace`.

See [conformance_prettier_ts_comments.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier_ts_comments.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
