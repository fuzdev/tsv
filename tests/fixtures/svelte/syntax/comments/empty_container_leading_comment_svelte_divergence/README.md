# Parser divergence: a leading comment on an EMPTY container collects the comments after it

Svelte's `add_comments` (`svelte/.../1-parse/acorn.js`) assigns `node.leadingComments`
**before** it recurses, so by the time zimmerframe's `next()` runs `for (const key in
node)` the freshly-added `leadingComments` array is one of the node's own keys — and its
entries are objects carrying a `type` (`'Block'` / `'Line'`). zimmerframe visits anything
with a `type`, so **each leading comment is walked as if it were an AST node**, with the
node it leads as its `path.at(-1)` parent.

That is normally inert: the walk's trailing rule needs either a same-line `/^[,) \t]*$/`
gap (impossible here — the node the comment leads sits between it and whatever follows)
or `is_last_in_body`, which tests

```js
parent.body.indexOf(node) === parent.body.length - 1
```

A comment object is never *in* `parent.body`, so `indexOf` is `-1` — which equals
`length - 1` exactly when the container is **empty**. The comment object then reports
itself last-in-body and drains every remaining comment up to the container's own `end`
into its own `trailingComments`, producing a `trailingComments` array nested *inside* a
`leadingComments` entry. Both empty-container spellings reach it:

- `const arr = /* a1 */ [/* a2 */];` — an empty `ArrayExpression` (`elements`)
- `/* b1 */ {` … `/* b2 */` … `}` — an empty `BlockStatement` (`body`)

An empty `ObjectExpression` (`properties`) has the identical trigger; it is left out here
only because `{ /* c */ }` also carries a separate bracket-spacing difference
(see [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md#typescript)
§Empty-object comment bracket spacing), which this parser fixture avoids conflating.

**tsv attaches comments to wire nodes only**, so a comment never becomes a walk node and
never claims another comment. `a2` and `b2` land where the walk puts them once the comment
objects are not in it — `a2` beside `b1` on the block statement's own `leadingComments`,
`b2` leading the declaration that follows. Nothing is lost either way: every comment is in
the root `comments` array in both parsers, the distinct-comment set is identical, and
`ast_diff` confirms code equivalence.

The last statement is the **null control**: `const full = /* c1 */ { /* c2 */ a: 1 };` is
the same comment layout over a *non-empty* container, where `indexOf` is `-1` and
`length - 1` is `0`. It does not diverge, and it is what keeps this fixture a claim about
emptiness rather than about leading comments in general.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
