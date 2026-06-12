# Async Generic Arrow Params (TypeScript) — Svelte Divergence

Confirms the acorn-typescript param-dropping bug exists in the standalone `.ts`
parsing path (acorn-typescript directly), not just through Svelte's parser wrapper.

See `basic_svelte_divergence/` for comprehensive type parameter pattern coverage.
This fixture is intentionally small — representative cases only.

**Bug**: acorn-typescript drops all function parameters from `async` arrow functions
that have type parameters (`async <T>(x: T) => x` → `params: []`).

**tsv** correctly includes the parameters.

**Upstream**: acorn-typescript — bug in async arrow parsing when type parameters are present.
