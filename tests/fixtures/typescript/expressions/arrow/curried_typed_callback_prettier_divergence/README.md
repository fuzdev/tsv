# Curried Typed Callback — Prettier Divergence

Tests curried arrow functions with generic type parameters, including callback
body wrapping when the body exceeds print width, and a final case combining
`async` with generics in a curried context.

## Formatter (Prettier) — forced trailing comma

Each arrow has a single unconstrained type param, so prettier forces `<T,>` (TSX
disambiguation) while tsv emits bare `<T>` — see single_type_param_prettier_divergence.
The `<T>` sits on its own line, above the body lines, so the comma does not affect body
wrapping. `output_prettier.svelte` records prettier's forced-comma output.

Reason: **Design choice** (formatter). See
[conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript.
