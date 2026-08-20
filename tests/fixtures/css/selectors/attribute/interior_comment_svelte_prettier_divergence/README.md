# interior_comment_svelte_prettier_divergence

Every gap *inside* an attribute selector admits a comment. Per
[css-syntax-3 §4](https://drafts.csswg.org/css-syntax/#consume-comment) a comment is
consumed at tokenization and produces **no token, not even whitespace**, and selectors-4's
[`<attribute-selector>`](https://drafts.csswg.org/selectors/#typedef-attribute-selector)
(`'[' <wq-name> ']' | '[' <wq-name> <attr-matcher> [<string-token>|<ident-token>]
<attr-modifier>? ']'`) is a token-level production, so every juncture in it is an
inter-token position:

- after the opening bracket — `[/* c1 */ attr]`
- between the name and the matcher — `[attr /* c2 */ ='value']`
- between the matcher and the value — `[attr= /* c3 */ 'value']`
- between the value and the closing bracket — `[attr='value' /* c4 */]`
- between the value and the case flag — `[attr='value' /* c5 */ i]`
- between the case flag and the closing bracket — `[attr='value' i /* c6 */]`
- the one interior gap of a bare presence selector — `[attr /* c7 */]`

Two comments in one gap separate single-spaced (`c8`/`c9`), and a `]` inside the comment
content is comment content, never the closing bracket.

## The two spacing rules

The brackets bound the selector, so a space in these gaps is **meaning-preserving**: the
comment takes a single space on each side, glued only to `[` and `]` themselves — the same
answer `::part()` and `:is()` give inside their parens. Only the gap holding the comment
changes; every other gap keeps its canonical (empty) spelling, which is why
`[attr /* c2 */ ='value']` pads the name→matcher gap and leaves `='value'` closed up.

The **whitespace-forbidden** gaps are the control, and take the opposite answer for the
same reason `.a/* c */.b` stays glued: selectors-4 forbids white space "between **any** of
the components of a `<wq-name>`" and "between the components of an `<attr-matcher>`", so a
space there would be a `<whitespace-token>` the grammar rejects, while a comment is no
token at all. Glued, therefore:

- the `<wq-name>` separator — `[svg/* c10 */|attr]`, `[svg|/* c11 */attr]`, `[*/* c12 */|attr]`,
  `[|/* c13 */attr]`
- the `<attr-matcher>` interior — `[attr~/* c14 */='value']`, and `[attr|/* c15 */='value']`, where the
  very same `|` that prefixes a namespace in `c11` is instead the matcher's first
  component. Neither `~=` nor `|=` is a token: css-syntax-3 gives U+007E and U+007C no case
  in *consume a token*, so both are plain `<delim-token>`s, its serialization table lists
  no pair among them needing separation, and preserved comments "may be reinserted even if
  the … tables don't require a comment between two tokens" — which is exactly the
  round-trip guarantee that makes the split legal.

The bare type-selector spelling of the `<wq-name>` rule is
[separator_comment](../../namespace/separator_comment_svelte_divergence/).

## Svelte divergence

Svelte's `parseCss` rejects a comment in every one of these positions
(`css_expected_identifier` — its attribute-selector reader does not tokenize comments), so
`expected_svelte.json` records the parse error. tsv accepts, per the spec — the same
canonical-fails-tsv-ok shape as
[combinator_comment](../../combinator_comment_svelte_prettier_divergence/) and
[part_interior_comment](../../pseudo_element/part_interior_comment_svelte_prettier_divergence/).

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Prettier divergence

Prettier prints **any** selector containing a comment verbatim — `parseSelector` returns
`selector-unknown` on the first `/*`, before its selector parser runs ("the parser tries to
parse the content of the comment as selectors which turns it into complete garbage"). So
every spelling here is prettier-stable and prettier's agreement with `input.svelte` is a
parser give-up rather than a rule about attribute selectors; the divergence is reachable
only from an authoring tsv normalizes. `prettier_variant_compact` pins the glued forms and
`prettier_variant_spaces` the padded ones — prettier keeps both, tsv normalizes both to
`input.svelte`.

The `c8`/`c9` pair keeps its space in the compact variant, and the whitespace-forbidden
gaps keep their glued spelling in the spaces variant: gluing two comments into a run makes
prettier relocate the rule's `{` onto its own line, a separate quirk pinned by
[separator_comment_run](../../namespace/separator_comment_run_svelte_prettier_divergence/)
and [compound_comment_run](../../compound_comment_run_svelte_prettier_divergence/).

See [conformance_prettier_css.md §CSS: Comments](../../../../../../docs/conformance_prettier_css.md#css-comments).

## Fixture Structure

- `expected_ours.json` — tsv's AST (source of truth)
- `expected_svelte.json` — Svelte's rejection
- `prettier_variant_compact.svelte` / `prettier_variant_spaces.svelte` — the glued and
  padded forms prettier keeps stable and tsv normalizes to input
- `input_invalid_whitespace_in_attr_matcher.svelte` — the boundary the glued rule rests on:
  a `<whitespace-token>` between the matcher's components is rejected by both parsers

## Related

- [separator_comment](../../namespace/separator_comment_svelte_divergence/) — the `<wq-name>` separator, glued, in its bare type-selector spelling
- [separator_comment_run](../../namespace/separator_comment_run_svelte_prettier_divergence/) — a glued **run** in the whitespace-forbidden gaps
- [namespace](../namespace_svelte_divergence/) — attribute namespaces without comments
- [flags](../flags/) — the case flags without comments
