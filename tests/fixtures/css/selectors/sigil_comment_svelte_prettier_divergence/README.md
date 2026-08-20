# sigil_comment_svelte_prettier_divergence

A comment between a selector's **sigil** and the name it introduces. selectors-4's
[grammar](https://drafts.csswg.org/selectors/#grammar) writes each of these as two
components — `<class-selector> = '.' <ident-token>`, `<pseudo-class-selector> = ':'
<ident-token> | ':' <function-token> <any-value> ')'`, `<pseudo-element-selector> = ':'
<pseudo-class-selector>` — and its *white space is forbidden* list names exactly these
junctures:

> Between **any** of the components of a `<type-selector>` or a `<class-selector>`.
> Between the ':'s, or between the ':' and `<ident-token>` or `<function-token>`, of a
> `<pseudo-element-selector>` or a `<pseudo-class-selector>`.

A `/* */` is not a `<whitespace-token>` — per
[css-syntax-3 §4.3.2](https://drafts.csswg.org/css-syntax/#consume-comment) *consume
comments* runs at the start of *consume a token* and **returns nothing** — so the comment
is admitted where the space is not, and it stays **glued**. Exactly the rule the
`<wq-name>` separator takes in
[separator_comment](../namespace/separator_comment_svelte_divergence/), one production over:

- `.` → class name — `./* c1 */cls`
- `:` → pseudo-class name — `:/* c2 */hover`
- `::` → pseudo-element name — `::/* c3 */before`
- between the two colons — `:/* c4 */:before`
- the functional form, where the name still ends at the `(` — `:/* c5 */not(.a)`

`#id` is **not** one of these: a `<hash-token>` is a single token, so there is no juncture
to split. Neither is an ident glued to `(` (`:not/* c */(`), a single `<function-token>`
that both parsers correctly reject.

## The name is where the parser says, not where a scan guesses

Every consumer of a pseudo's name (the case fold, the wire's half-decode, the scoping
compiler's `:global` test) starts it at `comments::pseudo_name_start` rather than at a
fixed one- or two-byte sigil, and a class name at `class_name_start`. Each step is
anchored at a known position instead of searching, which keeps it escape-proof — the same
discipline the `<wq-name>` separator needs, where an escaped `|` inside the prefix would
answer a search wrong
([namespace_escaped_prefix](../attribute/namespace_escaped_prefix_svelte_prettier_divergence/)).

## Svelte divergence

Svelte's `parseCss` rejects a comment at every one of these junctures
(`css_expected_identifier` — its selector reader does not tokenize comments there), so
`expected_svelte.json` records the parse error. tsv accepts, per the spec — the same
canonical-fails-tsv-ok shape as
[combinator_comment](../combinator_comment_svelte_prettier_divergence/).

Because the oracle rejects the whole stylesheet, tsv's **compiler** must not compile one
either: `css_scope`'s `refuse_if_comment` refuses these selectors, so accepting them in
the parser adds no over-acceptance to the refusal contract.

See [conformance_svelte.md §CSS Corrections](../../../../../docs/conformance_svelte.md#css-corrections).

## Prettier divergence

Prettier prints **any** selector containing a comment verbatim — `parseSelector` returns
`selector-unknown` on the first `/*`, before its selector parser runs — so it agrees with
tsv on every glued form here, and `input.svelte` is prettier-stable. The divergence is
reachable only through the case fold: tsv lowercases a pseudo-class keyword
(`:HOVER` → `:hover`) and keeps doing so with a comment in front, folding the **name**
and leaving the comment's content alone; prettier's freeze stops it folding at all.
`prettier_variant_uppercase_pseudo_name` pins that form — stable under prettier, normalized
by tsv to `input.svelte`.

See [conformance_prettier_css.md §CSS: Comments](../../../../../docs/conformance_prettier_css.md#css-comments).

## Fixture Structure

- `expected_ours.json` — tsv's AST (source of truth); every `name` excludes the comment
- `expected_svelte.json` — Svelte's rejection
- `prettier_variant_uppercase_pseudo_name.svelte` — `:/* C6 */HOVER`, which prettier keeps
  stable and tsv folds to `input.svelte`
- `input_invalid_whitespace_after_class_sigil.svelte` / `input_invalid_whitespace_between_colons.svelte`
  — the boundary the glued rule rests on: a `<whitespace-token>` at these junctures is
  rejected by both parsers

## Related

- [separator_comment](../namespace/separator_comment_svelte_divergence/) — the `<wq-name>` separator, the same glued rule
- [combinator_comment](../combinator_comment_svelte_prettier_divergence/) — the gaps *between* simple selectors, where the comment normalizes to single-space separation instead
- [interior_comment](../attribute/interior_comment_svelte_prettier_divergence/) — the attribute selector's spacing-safe gaps, which pad
- [pseudo_escaped_case](../pseudo_escaped_case/) — the case fold without comments
