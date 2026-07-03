# nth_child_negative_svelte_divergence

Several **spec-valid** negative `An+B` forms are over-rejected by Svelte's `parseCss`
(`css_expected_identifier`), while tsv and prettier accept them:

- `:nth-child(-3)` — a bare negative `<integer>` (`A=0, B=-3`)
- `:nth-child(-2n)` — a negative `<n-dimension>` (`A=-2, B=0`)
- `:nth-child(-2n - 3)` — negative `A` with a negative offset (`A=-2, B=-3`)
- `:nth-child(-0)` — negative-zero `<integer>`

Per [css-syntax-3 §the-anb-microsyntax](https://drafts.csswg.org/css-syntax/#anb-microsyntax)
the `<an+b>` grammar admits a signed `<integer>` (so `-3` and `-0` are valid), a negative
`<n-dimension>` (`-2n`), and `<n-dimension> ['+' | '-'] <signless-integer>` (`-2n - 3`).
Svelte's `:nth-child` reader accepts a leading-`-n` coefficient (`-n`, `-n - 3`) and any
`+`-tailed form (`-2n + 1`) but rejects the negative-coefficient and bare-negative-integer
forms above. tsv follows the spec (matching prettier), so it parses them where Svelte fails.

Formatting matches prettier — the canonical spaced forms are stable in both — so this is a
pure parse/AST divergence (`_svelte_divergence`, no `output_prettier.svelte`). (tsv's
`:nth-child` An+B reader is deliberately lenient/spec-following; the strict reader used for a
bare `An+B` term in `:is()`/unknown-pseudo args, a port of Svelte's `REGEX_NTH_OF`, rejects
these there — but that context is itself an over-acceptance of Svelte's, not the `:nth-child`
An+B grammar.)

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Fixture Structure

- `expected_ours.json` — tsv's output (source of truth; parses all four forms)
- `expected_svelte.json` — the error marker (Svelte's `parseCss` rejects the file)
