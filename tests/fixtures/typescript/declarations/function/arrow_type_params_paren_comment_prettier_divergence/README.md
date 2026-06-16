# Arrow type params paren comment divergence

Two prettier divergences stack on this one arrow:

1. **Comment position.** Prettier moves a block comment between the type-param
   `>` and the opening `(` inside the parens, as a leading comment on the first
   parameter. tsv preserves the comment between `>` and `(`.
2. **Trailing comma.** Prettier forces a `<T,>` trailing comma on the single
   unconstrained type param; tsv emits bare `<T>` (see
   single_type_param_prettier_divergence).

- tsv: `<T> /* c */(x: T) => x`
- Prettier: `<T,>(/* c */ x: T) => x`

Per comment-placement policy tsv preserves the user's comment position, and per
the no-JSX design choice it drops the trailing comma. Because both differ, no
single form is stable in both formatters — `unformatted_ours_compact.svelte`
normalizes to the tsv form, and prettier reaches `output_prettier.svelte`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §TypeScript.
