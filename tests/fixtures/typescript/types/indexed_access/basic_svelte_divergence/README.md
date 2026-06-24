# Indexed Access Basic — Svelte Divergence

Tests indexed access types (`T[K]`, `T[keyof T]`, tuple access, conditional types,
mapped types, etc.) including their use in async generic arrow return types.

**Bug**: acorn-typescript drops all function parameters from `async` arrow functions
that have type parameters. This affects the `asyncFn` example:

```ts
const asyncFn = async <T, K extends keyof T>(obj: T, key: K): Promise<T[K]> =>
	obj[key];  // acorn-typescript: params: [] (both obj and key dropped)
```

The non-async generic arrow (`fn`) in the same fixture parses correctly — only the
`async` + type parameter combination triggers the bug.

**tsv** correctly includes the parameters.

**Upstream**: acorn-typescript — bug in async arrow parsing when type parameters are present.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.
