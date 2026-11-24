# Parser divergence: an `async <T>(…)` arrow's head comments attach in the arrow's one key order

tsv's wire gives every node type one key order. `ArrowFunctionExpression`'s is the order
acorn-typescript writes on its common routes (`parseParenArrowList`,
`parseSubscriptAsyncArrow`, the `<T>(…) => …` graft): `returnType` first, then `id`,
`expression`, `generator`, `async`, `params`, `body`, and `typeParameters` last. The
one route that writes another order is the generic async arrow, `async <T>(…) => …`
(`tsTryParseGenericAsyncArrowFunction`): `typeParameters`, `params`, `returnType`, then
the rest.

The order is not only cosmetic here. Svelte attaches comments with a walk over the tree
in key order (`add_comments` in `svelte/.../1-parse/acorn.js`, walked by zimmerframe's
`for (const key in node)`), and a node's leading comments are every queued comment that
starts before it when the walk enters it. So on this one route the head's comments go to
different nodes:

| comment | Svelte (route order) | tsv (the arrow's one order) |
| --- | --- | --- |
| `c1` — `(x /* c1 */): T` | `x`'s `trailingComments` | the return type's `leadingComments` |
| `c2` — `(x = /* c2 */ 1): T` | `1`'s `leadingComments` | the return type's `leadingComments` |
| `c3` — `async /* c3 */ <T …>` | the type parameters' `leadingComments` | the return type's `leadingComments` |
| `c4` — `(x): /* c4 */ T` | `T`'s `leadingComments` | the same — inside the return type either way |
| `c5` — `</* c5 */ T …>() => x` | `T`'s `leadingComments` | the body `x`'s `leadingComments` |
| `c6` — `<T extends A /* c6 */>(x) => x` | `T`'s `trailingComments` | the parameter `x`'s `leadingComments` |
| `c7` — `</* c7 */ T …>() => {}` | `T`'s `leadingComments` | the body block's `leadingComments` |

With a return type the walk enters it first, so every comment ahead of it in the head
leads it — what acorn's own order does on the common route, where
`(x /* c */): T => x` attaches `c` the same way in both parsers. Without one, every
comment ahead of the parameters — before or inside the type parameters — leads the first
parameter, or the body when there is none (`e`, `f`, `g`). Every comment is kept,
on a node of the same arrow, and in the root `comments` array of both parsers; the
formatter locates comments by position and is unaffected.

The `d` declaration is the control: its comment sits inside the return type, which no
order changes.

See [conformance_svelte.md](../../../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
