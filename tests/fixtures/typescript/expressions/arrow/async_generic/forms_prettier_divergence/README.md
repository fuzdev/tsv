# Async generic arrow — additional forms (Prettier divergence)

Three async-generic-arrow forms, each exercising a path the minimal
`minimal_prettier_divergence` and the per-axis siblings don't cover. The same
formatter divergence applies throughout — prettier's forced `<T,>` — but on
distinct constructs:

- `withOptional` — `async <T>(x?: T): Promise<T | undefined> => x`. An **optional**
  param node, distinct from the plain param (`minimal_`) and the rest param (`long`),
  on an arrow whose return type is a union. The single unconstrained `<T>` takes
  prettier's `<T,>`.
- `objectBody` — `async <T>(): Promise<T> => ({}) as T`. An arrow whose body is a
  parenthesized object literal with an `as` assertion.
- `typed` — `const typed: <T>() => Promise<T> = async <T>() => ({}) as T`. The
  annotation `<T>` is a **type** position and stays bare in BOTH tools; only the
  initializer `<T>` (value position) takes prettier's `<T,>`. The contrast pins
  that the divergence is value-position-specific.

## Formatter (Prettier) — forced trailing comma

Prettier forces a `<T,>` trailing comma on a single unconstrained type param
(the TSX disambiguation), while tsv emits the bare `<T>`. `output_prettier.svelte`
records prettier's forced-comma output; `unformatted_ours_*` variants normalize
to the bare input under tsv only.

Reason: **Design choice** (formatter). See
[conformance_prettier_ts.md](../../../../../../../docs/conformance_prettier_ts.md) §TypeScript.

## Siblings

- `minimal_prettier_divergence/` — the canonical `async <T>(x: T)` construct.
- `basic_ts/` — the standalone `.ts` path, where prettier keeps `<T>` bare and no divergence applies.
- `long/` — type-parameter width wrapping in Svelte context.
- `param_decorator_svelte_divergence/` — the one arrow form acorn-typescript accepts a param decorator on.
- `../../generic/single_type_param_prettier_divergence/` — the `<T,>` trailing-comma divergence (single + default-only, `<script>` and template).
