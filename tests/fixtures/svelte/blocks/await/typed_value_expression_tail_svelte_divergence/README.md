# `{:then}` value annotation with a `satisfies` tail — Svelte Divergence

`{:then value: T satisfies U}`. Canonical accepts it; **tsv rejects** it (`Expected '}'`).

Svelte's `read_type_annotation` (`1-parse/read/context.js`) reads the annotation as the tail
of a synthetic `_ as T satisfies U` expression. The emitted `TSTypeAnnotation` spans
`: T satisfies U`, but its `typeAnnotation` is the outer `satisfies` type only, `U`. The
author's `T` is not in the tree, and prettier-plugin-svelte, printing from it, writes
`{:then value: U}`.

`T satisfies U` is not a type, and tsc's parser rejects it in every context (TS1005). tsv's
annotation is a type parse, so it ends at `T`, and the head then finds `satisfies` where its `}`
belongs. `expected_svelte.json` records canonical's lossy tree.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
