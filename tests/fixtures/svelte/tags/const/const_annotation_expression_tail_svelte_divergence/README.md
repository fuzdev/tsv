# `{@const}` annotation with an `as` tail — Svelte Divergence

`{@const a: T as U = expr}`. Canonical accepts it; **tsv rejects** it (`Expected token =`).

Svelte's `read_type_annotation` (`1-parse/read/context.js`) reads the annotation as the tail
of a synthetic `_ as T as U = expr` expression, re-parsed up to its `=`. The emitted
`TSTypeAnnotation` spans `: T as U`, but its `typeAnnotation` is the outer assertion's type
only, `U`. The author's `T` is not in the tree.

`T as U` is not a type, and tsc's parser rejects it in every context (TS1005). tsv's
annotation is a type parse, so it ends at `T`, and the declarator then finds `as` where its `=`
belongs. `expected_svelte.json` records canonical's lossy tree.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
