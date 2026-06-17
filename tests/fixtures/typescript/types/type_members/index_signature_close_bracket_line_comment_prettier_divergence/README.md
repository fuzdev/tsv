# Divergence: line comments in the index-signature key-type→`]` gap

Line comments between an index signature's key type and its closing `]`
(`[key: string // b\n\t// d\n]`).

Prettier keeps the bracket inline and **relocates** an own-line comment to after
`]`, pushing the value `:` to its own line; tsv breaks the bracket and preserves
each comment where the author wrote it.

```ts
// prettier (relocates own-line → after ])   // tsv (preserves placement)
[                                            [
	key: string // b                            key: string // b
] // d                                        // d
: number;                                    ]: number;
```

A **same-line** trailing comment (`// b`) trails the type in both formatters —
that part matches; the divergence is the **own-line** comment (`// d`), which
prettier moves to after `]`. A block comment in this gap (`[key: string /* c */]`)
stays inline in both and is not a divergence. Same rule as every other
open/close delimiter — see
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy. Without the break, a line
comment here would swallow `]` and the value annotation (content loss).
