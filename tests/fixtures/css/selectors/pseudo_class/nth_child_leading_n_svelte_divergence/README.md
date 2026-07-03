# nth_child_leading_n_svelte_divergence

Svelte's `parseCss` `:nth-child` reader mis-parses leading-`-n` `An+B` forms instead of
reading a single `Nth`, while tsv (following css-syntax-3) reads each as one `Nth`:

- `:nth-child(-n)` — Svelte reads a `TypeSelector` named `-n`; tsv reads `Nth "-n"` (`A=-1, B=0`)
- `:nth-child(-n - 3)` — Svelte flattens into `TypeSelector "-n"` + a descendant `Combinator` +
  `TypeSelector "-"` + … ; tsv reads `Nth "-n - 3"` (`A=-1, B=-3`)

Per [css-syntax-3 §the-anb-microsyntax](https://drafts.csswg.org/css-syntax/#anb-microsyntax)
`-n` is a valid `<an+b>` (`A=-1, B=0`) and `-n ['+' | '-'] <signless-integer>` covers `-n - 3`
(`A=-1, B=-3`). Svelte's reader recognizes the `+`-tailed form (`-n + 6` → a single `Nth`) but
not the bare or `-`-tailed leading-`-n` forms, falling back to type-selector parsing. tsv
follows the spec (matching prettier), so it reads a clean `Nth` where Svelte produces a
malformed selector list.

Formatting matches prettier — the spaced forms are stable in both — so this is a pure
parse/AST divergence (`_svelte_divergence`, no `output_prettier.svelte`).

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Fixture Structure

- `expected_ours.json` — tsv's output (source of truth; one `Nth` per form)
- `expected_svelte.json` — documents Svelte's mis-parse (type selectors + combinators)
