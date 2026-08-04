# media_query_closing_comma_prettier_divergence

A comma **closing** a `<media-query-list>` (`@media screen,`) is a separator with no
entry after it, so tsv deletes it. Prettier keeps it — `prettier_variant_closing_comma`
is the form prettier holds stable that tsv normalizes to `input`.

tsv: `@media screen {`
Prettier: `@media screen, {`

CSS Syntax 3 §"parse a comma-separated list of component values" — the algorithm
mediaqueries-4 §Syntax delegates to — consumes up to each separator, discards it, and
stops once the input is empty. A final comma therefore produces no further entry:
`screen,` and `screen` are the *same* one-entry list, so deleting it leaves the parse
untouched.

This is the **only** construct whose closing comma tsv deletes, and tsv's standing rule
runs the other way: a comma the author wrote is a token, and tsv re-spells the tokens it
was given. In a declaration value the same comma is therefore kept (`transition: a,`,
`--x: a,` — see [comma_closing](../../values/lists/comma_closing_prettier_divergence/)),
because a declaration is matched against a grammar rather than split, so the token
carries meaning: css-values-4 §"Component value combinators" requires a comma to be
omitted when "all items following the comma have been omitted", making `transition: a,`
and `transition: a` two different declarations (the UA drops one and applies the other);
and a custom property, whose value is a verbatim token sequence, simply substitutes
different tokens.

The `<media-query-list>` is the one production the spec defines *as* the split itself, so
the token carries no meaning to lose — `screen,` and `screen` are the same one-entry
list. That is what earns the deletion here, and nothing weaker would: it is a real spec
difference, not a preference for tidier output.

Prettier is inconsistent with itself rather than with the spec: it drops the closing
comma in `@import` position (`@import url('a.css') screen,;` → `screen;`, where tsv
agrees — [import_media_query_empty](../import_media_query_empty/)) and keeps it in
`@media`. tsv applies one rule in both.

The deletion is conditioned on the parse being unchanged, which is not the same as
"always". A list whose **last entry is empty** cannot be spelled without the extra
comma, and there deleting it changes what the list means — `@import url('a.css'),;`
is the one-entry list `not all`, and without its comma it is the *empty* list, which
mediaqueries-4 evaluates to **true** (the import would flip from never applying to
always). Those are kept; see
[media_query_empty](../media_query_empty_prettier_divergence/).

## Reason

**Spec precedence.** See
[conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
(`Media query closing comma`).

## Related

- [media_query_empty](../media_query_empty_prettier_divergence/) — the empty entries the comma is kept for
- [import_media_query_empty](../import_media_query_empty/) — the `@import` side, where the two agree
- [comma_closing](../../values/lists/comma_closing_prettier_divergence/) — the same comma in a declaration value, where tsv keeps it
- [comma_trailing_empty_element](../../values/lists/comma_trailing_empty_element_prettier_divergence/) — one comma further, where the last element is empty
