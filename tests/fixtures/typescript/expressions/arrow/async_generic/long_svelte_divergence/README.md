# Async Generic Arrow Params — Svelte Divergence

**Bug**: acorn-typescript drops all function parameters from async generic arrow functions.

```ts
// Non-async generic arrow: params correctly parsed
const f = <T,>(x: T): T => x;           // params: [Identifier("x")]

// Async generic arrow: params dropped
const f = async <T,>(x: T): Promise<T> => x;  // params: [] (bug)
```

The `async` keyword combined with type parameters triggers the bug. Type parameters
and return type annotations are parsed correctly — only the function parameters are lost.

**tsv** correctly includes the parameters. This is a correction, not a compat behavior,
because the missing params corrupt the AST semantics (tools would think the function
takes zero arguments).

**Upstream**: acorn-typescript — the bug is in async arrow parsing when type parameters are present.
