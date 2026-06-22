# destructure_empty_prettier_divergence

An **empty** object-destructure pattern in a `{#each … as PATTERN}` binding keeps
tight braces in tsv: `{}` — never `{ }`. Every non-empty pattern spaces its braces
in both formatters now (`{a}` → `{ a }`), matching prettier-plugin-svelte; the empty
pattern is the lone remaining bracket-spacing divergence. prettier-plugin-svelte
inserts a space into the empty braces (`{ }`).

tsv: `{#each items as {}}` (tight)
Prettier: `{#each items as { }}` (spaced)

## Reason

**Design choice.** tsv's empty object braces stay tight everywhere — a TypeScript
empty destructure (`const {} = x`) and an empty object literal (`{}`) both keep
tight braces, and `bracketSpacing` only ever inserts spaces around content. An empty
pattern has no content to space, so tsv emits `{}` in this binding position too, for
one consistent empty-braces rule. prettier-plugin-svelte instead emits `{ }` for the
empty pattern (while agreeing on `{}` for the TS-side `const {} = x`).

The same divergence holds for the `{#await … then}` / `{:then}` / `{:catch}` binding
positions, which route through the same printer: prettier emits `{ }`, tsv `{}`
(prettier prints the empty pattern cleanly — no throw, no content loss). Not
separately fixtured here.

See
[conformance_prettier.md §Svelte: empty destructuring brace spacing](../../../../../../docs/conformance_prettier.md#svelte-empty-destructuring-brace-spacing).
