# angle_glued_block_comment_prettier_divergence

A run of block comments leading an angle-bracket type assertion's type (`<T>expr`), in the
broken-cast path (a line comment in the `<`→type gap is what forces it).

`input.ts` rather than `input.svelte`: the angle assertion is TS-only *syntax* that a `.svelte`
`<script lang="ts">` cannot host — the Svelte parser reads a leading `<` in expression position
as a tag.

The **run itself matches prettier**: a pair the author glued stays glued and the type breaks
below (`a`), and blocks the author put on their own lines keep them (`b`) — prettier's
`printLeadingComment`, applied through tsv's one shared leading-comment emitter. The broken
cast is assembled as an already-broken group so the run's soft `line` is measured against the
cast rather than escaping to the enclosing assignment group, which would pull the type up onto
the comment's line.

## The divergence

Only the `<`-line comment: tsv keeps `// force` on the `<` line, prettier relocates it to its
own line as the type's leading comment. The open-delimiter trailing-comment divergence, in the
same family as the type-parameter list's `<` and the type-argument list's `<`.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
