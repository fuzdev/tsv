# Curried Typed Callback — Svelte Divergence

Tests curried arrow functions with generic type parameters, including print width
boundary behavior (100/101 chars) for callback body wrapping.

**Bug**: acorn-typescript drops all function parameters from `async` arrow functions
that have type parameters. The fourth test case combines `async` with generics in a
curried context:

```ts
const e =
	(b: A) =>
	async <T,>(fn: (c: B) => Promise<T>): Promise<T> =>  // acorn-typescript: params: [] (bug)
		b.c(async (tx) => fn(new C({a: tx, b: d})));
```

The non-async generic arrows (first three test cases) parse correctly — only the
`async` + type parameter combination triggers the bug.

**tsv** correctly includes the parameters.

**Upstream**: acorn-typescript — bug in async arrow parsing when type parameters are present.
