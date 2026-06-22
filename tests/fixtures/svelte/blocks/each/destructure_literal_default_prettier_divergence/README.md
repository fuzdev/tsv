# destructure_literal_default_prettier_divergence

Literal **default values** in a `{#each … as PATTERN}` binding normalize to tsv's
canonical form — string literals to single quotes (`"x"` → `'x'`, with
escape-minimizing keeping double quotes for `a'b`), numeric literals to canonical
shape (`0xFF` → `0xff`, `1.50` → `1.5`, `.5` → `0.5`, `1E10` → `1e10`, `0xFFn` →
`0xffn`). Normalization applies recursively to literals nested in object/array
default values (`{x: "y", z: 1.50}` → `{x: 'y', z: 1.5}`) and to array-pattern
defaults (`[a = "x"]` → `[a = 'x']`). Booleans, `null`, and regex literals are
already canonical and pass through unchanged. prettier-plugin-svelte preserves the
source token verbatim.

tsv: `{#each items as { a = 'x', b = 0xff }}` (normalized)
Prettier: `{#each items as { a = "x", b = 0xFF }}` (source-preserved)

Both formatters space the object braces; the divergence is solely the literal token —
`prettier_variant_source.svelte` carries the raw author tokens (`"x"`, `0xFF`, `1.50`,
`.5`, `1E10`) that prettier keeps stable and tsv normalizes back to `input.svelte`.
(Array patterns take no brace spacing in either formatter.)

## Reason

**Design choice.** tsv routes these binding patterns through its TypeScript printer,
so literal defaults get the same `singleQuote` + numeric normalization as every other
literal tsv emits (matching `{@const}`). prettier-plugin-svelte prints the pattern from
raw source, normalizing neither quotes nor numbers. See
[conformance_prettier.md §Svelte: destructuring literal normalization](../../../../../../docs/conformance_prettier.md#svelte-destructuring-literal-normalization).
