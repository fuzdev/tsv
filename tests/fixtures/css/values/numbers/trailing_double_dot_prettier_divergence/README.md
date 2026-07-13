# trailing_double_dot_prettier_divergence

A malformed number with a *consecutive* double dot (`1..`, `1..e-10`, `1..5`) is
accepted by `parseCss` (stored raw). tsv leaves it **verbatim** — the same as it
already leaves the other malformed-dot forms `..5`, `1.5.5`, and `1.px`
(`number_dot_ident`) — so its format is idempotent.

Prettier normalizes it: `1..` → `1`, `1..e-10` → `1e-10`, `1..5` → `1.5` (some in
two passes — see `output_prettier.svelte` / `audit_signature.txt`, prettier's own
transient non-idempotency here). tsv doesn't reach prettier's fixed point because
the number tokenizer only strips a *single* trailing dot (`1.` → `1`); stripping
it from `1..` and re-gluing the leftover (`1..e-10` → `1.e-10`) would re-parse as
a number and normalize again on the next pass, violating the F1 fixed-point
invariant. Leaving the whole malformed run verbatim avoids that and is uniform
with tsv's treatment of every other malformed-dot form.

tsv: `1..e-10` → `1..e-10` (verbatim)
Prettier: `1..e-10` → `1.e-10` → `1e-10`

See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values)
(`Trailing double-dot`).

## Fixture Structure

- `input.svelte` — the malformed double-dot forms (tsv-format-stable)
- `output_prettier.svelte` — prettier's normalization (`audit_signature.txt` pins
  its multi-pass chain, since prettier trims one dot per pass)
