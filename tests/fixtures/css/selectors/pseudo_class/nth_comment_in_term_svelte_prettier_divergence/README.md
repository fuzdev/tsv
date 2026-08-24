# nth_comment_in_term_svelte_prettier_divergence

A comment *inside* the An+B term of a `:nth-*()` argument is inter-token trivia, so the
term still reads as a single `Nth` and the gaps around the comment normalize like every
other selector gap:

- `:nth-child(2n /* c1 */ + 1)` — between the `<n-dimension>` and the sign
- `:nth-child(2n + /* c2 */ 1)` — between the sign and the `<signless-integer>`

Per [css-syntax-3 §4](https://drafts.csswg.org/css-syntax/#consume-comment) comments are
consumed at tokenization and produce **no token**, so the
[An+B microsyntax](https://drafts.csswg.org/css-syntax/#anb-microsyntax) never sees one:
`<n-dimension> ['+' | '-'] <signless-integer>` matches through it in either gap, as a run
(`2n /* c3 */ /* c4 */ + 1`), with the spec-only negative forms (`-2n /* c5 */ - 3`) and
with `of S`. Operator characters *inside* the comment content are never touched
(`/* a+b */`) — the comment is opaque to the operator respacing.

The rule has two boundary controls. `:nth-child(even /* c8 */ + 1)` — `even`/`odd` take no
tail, so this is not an An+B term at all and stays demoted to the ordinary selector-list
path (a type selector, a combinator and a bare `Nth`), like `:nth-child(even odd)`. And
`input_invalid_non_spec_grammar_comment` — the bare An+B terms `:is()`/`:not()` over-accept
read Svelte's `REGEX_NTH_OF` grammar, which stays **comment-blind** for `parseCss` parity
(An+B is not a selector, so there is no spec to follow there), so a split term is not a
term at all and `:not()`'s strict list rejects it in both parsers.

## Svelte divergence

Svelte's `parseCss` rejects a comment anywhere in `:nth-*()` args except before the An+B
(`css_expected_identifier` — its An+B scanner doesn't tokenize comments), so
`expected_svelte.json` records the parse error. tsv accepts, per the spec.

Where the term has no type-selector reading (`2n`, `-2n`) a comment-blind split term rejects
outright; where it has one (`n`, `-n`) it reads as a selector list instead — the same
accept-but-mis-parse shape as the leading-`-n` forms, and the reason `n /* c6 */ + 1` is
part of this fixture's claim.

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Prettier divergence

Prettier prints **any** selector containing a comment verbatim — its selector parser bails
before it ever reaches the An+B (`parseSelector`'s `selector-unknown` early return: "the
parser tries to parse the content of the comment as selectors which turns it into complete
garbage"). So the frozen spelling is a parser give-up, not a rule about An+B, and every
form here is prettier-stable: `prettier_variant_compact` pins the glued forms prettier
keeps and tsv normalizes to input. tsv normalizes the gaps instead — the same single-space
rule it already applies at every other selector-comment position, and the reason a comment
never changes whether the term's operators respace.

See [conformance_prettier_css.md §CSS: Comments](../../../../../../docs/conformance_prettier_css.md#css-comments).

## Fixture Structure

- `expected_ours.json` — tsv's AST (source of truth; one `Nth` per split term)
- `expected_svelte.json` — Svelte's rejection
- `prettier_variant_compact.svelte` — the glued forms prettier keeps stable
- `input_invalid_non_spec_grammar_comment.svelte` — the `REGEX_NTH_OF` boundary: both
  parsers reject a split term outside `:nth-*()`
- `unformatted_ours_glued_run.svelte` / `prettier_variant_glued_run.svelte` — a run **glued
  to itself** (`2n/* c3 *//* c4 */+1`) separates single-spaced, the same answer a glued run
  gets in a value list; prettier keeps the run glued and relocates the `{` to its own line
  instead, so its stable form needs its own file rather than the compact one

## Related

- [nth_comment](../nth_comment_svelte_prettier_divergence/) — comments in the gaps *around* the An+B
- [nth_child_leading_n](../nth_child_leading_n_svelte_divergence/) — the other accept-but-mis-parse `Nth` family
- [nth_child_negative](../nth_child_negative_svelte_divergence/) — the spec-only negative forms
