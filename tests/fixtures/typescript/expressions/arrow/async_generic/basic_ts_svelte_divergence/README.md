# Async Generic Arrow Params (TypeScript) — Svelte Divergence

Confirms the acorn-typescript param-dropping bug exists in the standalone `.ts`
parsing path (acorn-typescript directly), not just through Svelte's parser wrapper.

See `long_svelte_divergence/` for the Svelte-context param-drop plus type-param width
coverage, and `stacked_svelte_prettier_divergence/` for the minimal case where the
param-drop and prettier's `<T,>` forced trailing comma land on one construct.
This fixture is intentionally small — representative cases only.

**Bug**: acorn-typescript drops all function parameters from `async` arrow functions
that have type parameters (`async <T>(x: T) => x` → `params: []`).

**tsv** correctly includes the parameters.

**Upstream**: acorn-typescript — bug in async arrow parsing when type parameters are present.

See [conformance_svelte.md](../../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.
