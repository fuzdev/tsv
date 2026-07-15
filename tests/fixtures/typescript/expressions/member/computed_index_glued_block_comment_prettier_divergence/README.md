# computed_index_glued_block_comment_prettier_divergence

A run of block comments leading a computed member-access index (`obj[idx]`, and the optional
`obj?.[idx]`), in the bracket-break path (a line comment in an in-bracket gap is the only
trigger — an element access never breaks on width).

The **run itself follows prettier's rule**: a pair the author glued stays glued and the index
breaks below (`a`, `c`), and blocks the author put on their own lines keep them (`b`) — that is
prettier's `printLeadingComment`, applied through tsv's one shared leading-comment emitter.

## The divergence

Prettier empties the brackets: it hoists the whole in-bracket comment run *out* to before the
object, and strands the `[`-line comment between the object and its own index —

```ts
const a =
	/* c1 */ /* c2 */
	obj // force
	[idx];
```

Every comment changes what it reads as being about. `c1`/`c2` were written about `idx` and now
lead the entire expression; `// force` sat on the `[` and now trails `obj`. Splitting `obj`
from `[idx]` across a line is itself gratuitous. tsv keeps all three where the author put them,
inside the brackets they were written in.

This is the open-delimiter trailing-comment divergence (the `[`-line comment) plus the
computed-member hoist, both sanctioned; the optional `?.[` form takes the same path and shows
the same relocation.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
