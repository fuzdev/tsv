# separator_comment_svelte_divergence

A comment may split a `<wq-name>`'s namespace separator, on either side of the `|`. Per
[css-syntax-3 §4](https://drafts.csswg.org/css-syntax/#consume-comment) a comment is
consumed at tokenization and produces **no token, not even whitespace**, so `svg/* c1 */|rect`
tokenizes identically to `svg|rect`. That distinction is the whole rule: selectors-4 forbids
white space "between **any** of the components of a
[`<wq-name>`](https://drafts.csswg.org/selectors/#typedef-wq-name)", and a
`<whitespace-token>` there really is rejected (tsv rejects `svg |rect` and `svg| rect`) — a comment
is not one. tsv accepts a comment in both gaps of every prefix form:

- a named prefix — `svg/* c1 */|rect`, `svg|/* c2 */rect`
- the universal prefix — `*/* c3 */|div`, `*|/* c4 */div`
- the explicit no-namespace prefix — `|/* c5 */div`, and its universal `|/* c6 */*`

A `|` inside the comment content is comment content, never the separator (`svg/* | */|rect`).

Because a space in these gaps is exactly the token the grammar forbids, the comment stays
**glued** — the same reason a compound-internal comment stays glued in
[combinator_comment](../../combinator_comment_svelte_prettier_divergence/), where a space
would turn `.a/* c */.b` into a descendant. Glued is therefore the *only* spelling tsv
accepts here, which is why this fixture has no whitespace variants and no prettier claim:
prettier prints any comment-bearing selector verbatim, so it lands on the same output tsv
does. (Its agreement is a give-up rather than a rule — `parseSelector` returns
`selector-unknown` on the first `/*` — but with nothing to disagree about, there is no
divergence to record.)

## Svelte divergence

Svelte's `parseCss` rejects a comment in the separator (`css_expected_identifier` — its
selector reader does not tokenize comments there), so `expected_svelte.json` records the
parse error. tsv accepts, per the spec — the same canonical-fails-tsv-ok shape as
[no_namespace](../no_namespace_svelte_divergence/), which `parseCss` rejects even without a
comment.

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Fixture Structure

- `expected_ours.json` — tsv's AST (source of truth)
- `expected_svelte.json` — Svelte's rejection
- `input_invalid_whitespace_in_wq_name.svelte` — the boundary the glued rule rests on: a
  `<whitespace-token>` in the separator is rejected by both parsers

## Related

- [separator_comment_run](../separator_comment_run_svelte_prettier_divergence/) — a glued **run** in the same gaps, where prettier does diverge (it relocates the `{`)
- [interior_comment](../../attribute/interior_comment_svelte_prettier_divergence/) — the attribute selector's spacing-safe interior gaps, which take the opposite (padded) answer
- [no_namespace](../no_namespace_svelte_divergence/) — `|element` without comments
- [prefix](../prefix/) — `ns|element` without comments
