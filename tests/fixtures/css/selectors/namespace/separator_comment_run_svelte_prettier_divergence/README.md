# separator_comment_run_svelte_prettier_divergence

The multi-comment generalization of
[separator_comment](../separator_comment_svelte_divergence/): a run of two **glued**
comments in a whitespace-forbidden selector gap stays glued and is emitted verbatim. Per
[css-syntax-3 §4.3.2](https://drafts.csswg.org/css-syntax/#consume-comment) the loop
consumes consecutive comments as one stretch of no-token material, so `svg/* c1 *//* c2 */|rect`
tokenizes identically to `svg|rect` — while inserting a space *between* the two comments would
produce a `<whitespace-token>`, which selectors-4 forbids "between **any** of the components
of a `<wq-name>`" and "between the components of an `<attr-matcher>`". The run therefore
covers all three spellings of that rule:

- the bare type selector — `svg/* c1 *//* c2 */|rect`
- the same `<wq-name>` inside an attribute selector — `[svg/* c3 *//* c4 */|attr]`
- the `<attr-matcher>`'s own interior — `[attr~/* c5 *//* c6 */='value']`

This is the selector-gap counterpart of
[compound_comment_run](../../compound_comment_run_svelte_prettier_divergence/), where the
same "no space, or the token stream changes" rule keeps `.a/* c *//* d */.b` a compound.

## Svelte divergence

Svelte's `parseCss` rejects a comment in any of these gaps (`css_expected_identifier`), so
`expected_svelte.json` records the parse error. tsv accepts, per the spec.

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Prettier divergence

Prettier agrees on the selector text — it prints any comment-bearing selector verbatim
(`parseSelector` returns `selector-unknown` on the first `/*`) — but two adjacent comments
in a selector make it relocate the rule's opening `{` onto its own line
(`output_prettier.svelte`), the same stable quirk
[compound_comment_run](../../compound_comment_run_svelte_prettier_divergence/) pins. tsv
keeps `{` on the selector line. Only the brace position differs.

See [conformance_prettier_css.md §CSS: Comments](../../../../../../docs/conformance_prettier_css.md#css-comments).

## Fixture Structure

- `expected_ours.json` — tsv's AST (source of truth)
- `expected_svelte.json` — Svelte's rejection
- `output_prettier.svelte` — prettier's output: identical but for the three relocated `{`

## Related

- [separator_comment](../separator_comment_svelte_divergence/) — the single-comment forms, where prettier agrees outright
- [compound_comment_run](../../compound_comment_run_svelte_prettier_divergence/) — the `{` relocation, and the same glued-run rule inside a compound
- [interior_comment](../../attribute/interior_comment_svelte_prettier_divergence/) — the attribute selector's spacing-safe gaps, where a run separates single-spaced instead
