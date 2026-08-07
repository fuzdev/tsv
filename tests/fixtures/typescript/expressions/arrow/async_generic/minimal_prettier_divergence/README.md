# Async generic arrow — minimal Prettier divergence

The canonical `async <T>(x: T): Promise<T> => x` — an async arrow carrying both a
type parameter and a value parameter. Minimal by design; the surveys live in the
siblings (see below).

## Formatter (Prettier) — forced trailing comma

Prettier forces a `<T,>` trailing comma on a single unconstrained type param
(the TSX disambiguation), while tsv emits the bare `<T>`. `output_prettier.svelte`
records prettier's forced-comma output; `unformatted_ours_*` variants normalize
to the bare input under tsv only.

Reason: **Design choice** (formatter). See
[conformance_prettier_ts.md](../../../../../../../docs/conformance_prettier_ts.md) §TypeScript.

## Per-axis coverage (siblings)

- `forms_prettier_divergence/` — additional forms beyond this one construct: optional param (`x?`), object-`as`-literal body, and a type-vs-value-position `<T,>` contrast.
- `basic_ts/` — the standalone `.ts` path, where prettier keeps `<T>` bare and no divergence applies.
- `long/` — type-parameter width wrapping in Svelte context.
- `param_decorator_svelte_divergence/` — the one arrow form acorn-typescript accepts a param decorator on.
- `../../generic/single_type_param_prettier_divergence/` — the `<T,>` trailing-comma divergence (single + default-only, `<script>` and template).
