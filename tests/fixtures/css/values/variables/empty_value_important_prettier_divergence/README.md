# empty_value_important_prettier_divergence

An empty custom-property value carrying `!important` (`--a: !important;`)
normalizes to a single space, like other empty custom-property values
(see [empty_value](../empty_value_prettier_divergence/)).

Prettier is **non-convergent** on this input — it adds a space before
`!important` on every pass (`--a:!important` → `--a: !important` →
`--a:  !important` → …) and never reaches a fixed point, in both the standalone
CSS parser and the prettier-svelte plugin, so it can't serve as a formatter
oracle. tsv is stable and correct; prettier is the broken party.

`prettier_nonconvergent.txt` records the claim: no fixed point exists, so there
is no `output_prettier.css` to pin and no prettier-anchored variants are
possible. The validator live-verifies the non-convergence (rule F5) instead of
running F2/F3/F4 — if prettier ever converges here, validation fails with a
hint to re-document the fixture normally.

This is a `.css` fixture because the construct is pure CSS; the embedded path
is covered by the sibling `.svelte` fixtures.

## Reason

Prettier bug. See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values) ("Empty value + `!important`").
