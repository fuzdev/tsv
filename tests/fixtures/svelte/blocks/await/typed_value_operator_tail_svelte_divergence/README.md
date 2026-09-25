# `then` shorthand value annotation with an operator tail — Svelte Divergence

`{#await promise then value: T + 1}`. Canonical accepts it; **tsv rejects** it
(`Expected '}'`).

Svelte's `read_type_annotation` (`1-parse/read/context.js`) reads the annotation as the tail
of a synthetic `_ as T + 1` expression. acorn-typescript reads that as a `BinaryExpression`,
which has no `typeAnnotation` field. The emitted `TSTypeAnnotation` therefore spans `: T + 1`
and carries **no type at all**. prettier-plugin-svelte throws on that tree. Any binary, logical or
conditional operator does the same in every block binding position.

`T + 1` is not a type, and tsc's parser rejects it in every context (TS1005). tsv's annotation
is a type parse, so it ends at `T`, and the head then finds `+` where its `}` belongs.
`expected_svelte.json` records canonical's type-less tree.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
