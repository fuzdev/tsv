# Async generic arrow — stacked Svelte + Prettier divergence

A single `async <T>(x: T): Promise<T> => x` is the one construct where BOTH
async-generic-arrow divergences land on the same token sequence. Minimal by
design — the per-axis surveys live in the siblings (see below).

## 1. Parser (Svelte) — dropped params

acorn-typescript drops every function parameter from an `async` arrow that has
type parameters:

```ts
const f = <T>(x: T): T => x;                 // params: [Identifier("x")]
const f = async <T>(x: T): Promise<T> => x;  // params: [] (bug)
```

The `async` keyword combined with type parameters triggers it; type parameters
and the return annotation parse correctly — only the params are lost. **tsv**
keeps the param (`expected_ours.json`); `expected_svelte.json` records Svelte's
dropped-param AST. This is a correction, not a compat behavior — the missing
param corrupts the AST semantics.

**Upstream**: acorn-typescript — async arrow parsing when type parameters are present.

Reason: **Parser compat** (correction). See
[conformance_svelte.md](../../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.

## 2. Formatter (Prettier) — forced trailing comma

Prettier forces a `<T,>` trailing comma on a single unconstrained type param
(the TSX disambiguation), while tsv emits the bare `<T>`. `output_prettier.svelte`
records prettier's forced-comma output; `unformatted_ours_*` variants normalize
to the bare input under tsv only.

Reason: **Design choice** (formatter). See
[conformance_prettier.md](../../../../../../../docs/conformance_prettier.md) §TypeScript.

## Per-axis coverage (siblings)

- `basic_ts_svelte_divergence/` — the param-drop on the standalone `.ts` (acorn-typescript) path.
- `long_svelte_divergence/` — the param-drop in Svelte context plus type-param width wrapping.
- `../../generic/single_type_param_prettier_divergence/` — the `<T,>` trailing-comma divergence (single + default-only, `<script>` and template).
