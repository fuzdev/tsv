# Curried Typed Callback — Svelte + Prettier Divergence

Tests curried arrow functions with generic type parameters, including print width
boundary behavior (100/101 chars) for callback body wrapping. Two independent
divergences apply.

## 1. Parser (Svelte) — dropped params

**Bug**: acorn-typescript drops all function parameters from `async` arrow functions
that have type parameters. The fourth test case combines `async` with generics in a
curried context:

```ts
const e =
	(b: A) =>
	async <T>(fn: (c: B) => Promise<T>): Promise<T> =>  // acorn-typescript: params: [] (bug)
		b.c(async (tx) => fn(new C({a: tx, b: d})));
```

The non-async generic arrows (first three test cases) parse correctly — only the
`async` + type parameter combination triggers the bug. **tsv** correctly includes the
parameters (`expected_ours.json`; `expected_svelte.json` captures Svelte's dropped-param AST).

**Upstream**: acorn-typescript — bug in async arrow parsing when type parameters are present.

## 2. Formatter (Prettier) — forced trailing comma

Each arrow has a single unconstrained type param, so prettier forces `<T,>` (TSX
disambiguation) while tsv emits bare `<T>` — see single_type_param_prettier_divergence.
The `<T>` sits on its own line, above the width-boundary body lines, so the comma does not
shift the 100/101 boundary. `output_prettier.svelte` records prettier's forced-comma output.

Reason: **Design choice** (formatter). See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §TypeScript.
