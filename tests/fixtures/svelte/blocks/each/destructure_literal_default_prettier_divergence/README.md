# destructure_literal_default_prettier_divergence

Literal **default values** in a `{#each … as PATTERN}` binding normalize to tsv's
canonical form — string literals to single quotes (`"x"` → `'x'`, with
escape-minimizing keeping double quotes for `a'b`), numeric literals to canonical
shape (`0xFF` → `0xff`, `1.50` → `1.5`, `.5` → `0.5`, `1E10` → `1e10`, `0xFFn` →
`0xffn`). Normalization applies recursively to literals nested in object/array
default values (`{x: "y", z: 1.50}` → `{x: 'y', z: 1.5}`) and to array-pattern
defaults (`[a = "x"]` → `[a = 'x']`). Booleans, `null`, and regex literals are
already canonical and pass through unchanged. prettier-plugin-svelte preserves the
source token verbatim, the same way it preserves the spaced braces.

tsv: `{#each items as {a = 'x', b = 0xff}}` (normalized, hugged)
Prettier: `{#each items as { a = "x", b = 0xFF }}` (source-preserved, spaced)

## Reason

**Design choice.** tsv routes these binding patterns through its TypeScript printer,
so literal defaults get the same `singleQuote` + numeric normalization as every other
literal tsv emits (matching `{@const}`). prettier-plugin-svelte prints the pattern from
raw source, normalizing neither quotes nor numbers. Sibling of the bracket-spacing
[destructure_object_default](../destructure_object_default_prettier_divergence/)
divergence. See
[conformance_prettier.md §Svelte: destructuring literal normalization](../../../../../../docs/conformance_prettier.md#svelte-destructuring-literal-normalization).
