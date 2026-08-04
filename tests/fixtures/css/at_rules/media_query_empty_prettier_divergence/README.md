# media_query_empty_prettier_divergence

An **empty `<media-query>`** in a comma-separated list — a leading comma or two
adjacent ones — is a real entry in the list. tsv keeps it; prettier deletes it.

tsv: `@media , screen`, `@media screen, , print`, `@import url('./a.css'),`
Prettier: `@media screen`, `@media screen, print`, `@import url('./a.css')`

Media Queries Level 4 §Syntax parses a `<media-query-list>` by parsing "a
comma-separated list of component values, then parsing each entry as a
`<media-query>`", so every top-level comma is a separator and the (possibly empty)
text between two of them is an entry. §Error Handling then says an entry that
matches no `<media-query>` "must be replaced by `not all` during parsing", and that
a grammar mismatch "does **not** wipe out an entire media query list, just the
problematic media query". The empty entry is therefore `not all`, still occupying
its slot — deleting it shortens the authored list, which is a rewrite rather than a
normalization.

The `@import url('./a.css'),;` case shows why the rule cannot be "always drop the
trailing comma": that list is the single entry `not all`, which is **false**, and
without its comma it is the *empty* list, which mediaqueries-4 evaluates to **true**
— the import would flip from never applying to always. A closing comma the entries
*can* be spelled without is dropped, which is the sibling
[media_query_closing_comma](../media_query_closing_comma_prettier_divergence/).

## Reason

**Content preservation**, and in one case a **prettier bug**. See
[conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
(`Empty media query`).

Prettier is inconsistent with itself here as well as with the spec — it deletes an
empty query in `@media` position, but keeps an interior one in `@import` position
(`@import url('./a.css') screen, , print` survives its value parser intact, which is
where tsv and prettier agree — [import_media_query_empty](../import_media_query_empty/)).
tsv applies one rule in both positions, which is what makes those interior cases a
match rather than a second divergence.

As long as one other entry survives, dropping the empty one changes nothing that
renders: `not all` matches nothing, so `screen, not all, print` selects the media
`screen, print` selects. That is why the two `@media` cases here are cosmetic where
the corresponding **declaration-value** behavior is not — an empty element there
invalidates the whole declaration, so dropping it turns a dead declaration into a
live one.

The `@import` case is **not** cosmetic, and is a prettier bug rather than a taste
difference: there the empty entry is the list's *only* entry, so deleting it leaves
no `<media-query-list>` at all — the false→true flip above, arrived at from
prettier's side rather than tsv's. Same class of change as the declaration-value one,
in the other direction.

## Related

- [media_query_closing_comma](../media_query_closing_comma_prettier_divergence/) — the closing comma that *is* dropped
- [import_media_query_empty](../import_media_query_empty/) — the interior-empty cases, where tsv and prettier agree
- [import_media_query_leading_comma](../import_media_query_leading_comma/) — a leading comma with a real query after it, also a match
- [comma_empty_element](../../values/lists/comma_empty_element/) — the same construct in a declaration value
