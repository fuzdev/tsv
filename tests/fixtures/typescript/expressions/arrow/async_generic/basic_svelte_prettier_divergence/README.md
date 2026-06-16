# Async Generic Arrow Params — Svelte + Prettier Divergence

Two independent divergences apply to async generic arrows here.

## 1. Parser (Svelte) — dropped params

**Bug**: acorn-typescript drops all function parameters from async generic arrow functions.

```ts
// Non-async generic arrow: params correctly parsed
const f = <T>(x: T): T => x;                   // params: [Identifier("x")]

// Async generic arrow: params dropped
const f = async <T>(x: T): Promise<T> => x;    // params: [] (bug)
```

The `async` keyword combined with type parameters triggers the bug. Type parameters
and return type annotations are parsed correctly — only the function parameters are lost.
**tsv** correctly includes the parameters (recorded in `expected_ours.json`;
`expected_svelte.json` captures Svelte's dropped-param AST). This is a correction, not a
compat behavior, because the missing params corrupt the AST semantics.

**Upstream**: acorn-typescript — bug in async arrow parsing when type parameters are present.

## 2. Formatter (Prettier) — forced trailing comma

Prettier forces a `<T,>` trailing comma on single-unconstrained type params (the TSX
disambiguation), while tsv emits the bare `<T>` form — see
single_type_param_prettier_divergence. Only single-unconstrained (and default-only `<T = string>`)
params diverge; constrained and multi-param forms (`<T extends object>`, `<T, U>`) match.
`output_prettier.svelte` records prettier's forced-comma output; `unformatted_ours_*`
variants normalize to the bare input under tsv only.

Reason: **Design choice** (formatter). See
[conformance_prettier.md](../../../../../../../docs/conformance_prettier.md) §TypeScript.
