# Async generic arrow — additional forms (Svelte + Prettier divergence)

Three async-generic-arrow forms, each exercising a path the minimal
`stacked_svelte_prettier_divergence` and the per-axis siblings don't cover.
The same two divergences apply as everywhere in this group — the
acorn-typescript param-drop (parser) and prettier's forced `<T,>` (formatter) —
but on distinct constructs:

- `withOptional` — `async <T>(x?: T): Promise<T | undefined> => x`. The param-drop
  bug hits an **optional** param node too, a different node than the plain param
  (`stacked_`) and the rest param (`long_svelte_divergence`). tsv keeps it
  (`expected_ours.json`); Svelte drops it (`expected_svelte.json`). The single
  unconstrained `<T>` also takes prettier's `<T,>`.
- `objectBody` — `async <T>(): Promise<T> => ({}) as T`. No params, so nothing
  is dropped; only the `<T,>` formatter divergence applies, on an arrow whose
  body is a parenthesized object literal with an `as` assertion.
- `typed` — `const typed: <T>() => Promise<T> = async <T>() => ({}) as T`. The
  annotation `<T>` is a **type** position and stays bare in BOTH tools; only the
  initializer `<T>` (value position) takes prettier's `<T,>`. The contrast pins
  that the divergence is value-position-specific.

## 1. Parser (Svelte) — dropped params

acorn-typescript drops every function parameter from an `async` arrow that has
type parameters; the optional param above confirms it applies to `x?` as well.
This is a correction, not a compat behavior — the missing param corrupts the AST
semantics.

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

## Siblings

- `stacked_svelte_prettier_divergence/` — the minimal `async <T>(x: T)` where both divergences land on one construct.
- `basic_ts_svelte_divergence/` — the param-drop on the standalone `.ts` (acorn-typescript) path.
- `long_svelte_divergence/` — the param-drop in Svelte context plus type-param width wrapping.
- `../../generic/single_type_param_prettier_divergence/` — the `<T,>` trailing-comma divergence (single + default-only, `<script>` and template).
