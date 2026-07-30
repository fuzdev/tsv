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

tsv deletes a trailing comma in a declaration value too (`transition: a,` → `a`,
where prettier agrees, as it does in a JS list under `trailingComma: 'none'`), but
that is a weaker rule and not this one. A declaration value is matched against its
grammar rather than split, and css-values-4 §"Component value combinators" requires a
comma to be omitted when "all items following the comma have been omitted" — so
`transition: a,` is invalid where `transition: a` is valid, and the deletion turns a
dead declaration live. Only the `<media-query-list>` has the spec's own split
algorithm behind it, which is what makes the deletion parse-preserving here.

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
[conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules)
(`Media query closing comma`).

## Related

- [media_query_empty](../media_query_empty_prettier_divergence/) — the empty entries the comma is kept for
- [import_media_query_empty](../import_media_query_empty/) — the `@import` side, where the two agree
- [comma_trailing_empty_element](../../values/lists/comma_trailing_empty_element_prettier_divergence/) — the same predicate in a declaration value
