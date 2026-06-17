# destructure_literal_default_prettier_divergence

Literal **default values** in `{#await … then}`, `{:then}`, and `{:catch}` bindings
normalize to tsv's canonical form — string literals to single quotes (`"x"` → `'x'`,
with escape-minimizing keeping double quotes for `a'b`), numeric literals to canonical
shape (`0xFF` → `0xff`). prettier-plugin-svelte preserves the source token verbatim,
the same way it preserves the spaced braces. Mirrors the each-block
[destructure_literal_default](../../each/destructure_literal_default_prettier_divergence/)
across all three await binding positions.

tsv: `{:then {a = 0xff}}`, `{:catch {a = "a'b"}}` (normalized, hugged)
Prettier: `{:then { a = 0xFF }}`, `{:catch { a = "a'b" }}` (source-preserved, spaced)

## Reason

**Design choice.** tsv routes these binding patterns through its TypeScript printer,
so literal defaults get the same `singleQuote` + numeric normalization as every other
literal tsv emits (matching `{@const}`). prettier-plugin-svelte prints the pattern from
raw source, normalizing neither quotes nor numbers. Sibling of the bracket-spacing
[destructure_default](../destructure_default_prettier_divergence/) divergence. See
[conformance_prettier.md §Svelte: destructuring literal normalization](../../../../../../docs/conformance_prettier.md#svelte-destructuring-literal-normalization).
