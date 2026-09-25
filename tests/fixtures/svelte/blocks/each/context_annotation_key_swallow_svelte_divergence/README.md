# `{#each}` context annotation that swallows the key — Svelte Divergence

`{#each items as item: A ? B : C (item)}`. Canonical accepts it; **tsv rejects** it
(`Expected '}'`).

Svelte's `read_type_annotation` (`1-parse/read/context.js`) reads the annotation as the tail
of a synthetic `_ as A ? B : C (item)` expression. The conditional's last operand continues
into the key's parentheses as a **call**, `C(item)`. So canonical's `TSTypeAnnotation` spans
`: A ? B : C (item)` and carries no type (a `ConditionalExpression` has no
`typeAnnotation`), and the `EachBlock` has **no key**.

`A ? B : C` is not a type (a conditional type needs `extends`), and tsc's parser rejects the
annotation in every context (TS1005). tsv's annotation is a type parse, so it ends at `A`, and
the head then finds `?` where its `,`, `(` or `}` belongs. `expected_svelte.json` records
canonical's tree, key lost.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
