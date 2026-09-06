# media_feature_escaped_colon_prettier_divergence

A media-feature name carrying an **escaped `:`** — `@media (a\:b: 1px)`.

`\:` is a valid escape (CSS Syntax 3 §4.3.7) and §"Consume an ident sequence" takes its
payload as ident content, so `a\:b` is the single ident `a:b` and the **second** `:` is the
name→value separator. tsv reads the feature name as that whole run: a plain name is ASCII
case-insensitive (mediaqueries-5 §"Media Features") so it lowercases entire, and a
custom-media name is case-sensitive (css-variables-1) so `--A\:B` survives whole.

Prettier's media-query parser reads the escape's payload as the separator, ending the name
at `a\:` and re-emitting the rest as the start of the value — `(a\: b: 1px)`, inserting a
space that is not in the author's ident and leaving `B` uppercase because it now sits in
value position. The space is content: `a\: b` is the ident `a:` followed by the ident `b`,
where the author wrote one ident. Prettier's form is its own fixed point, so a second pass
raises nothing.

`divergent_variant_uppercase.svelte` is what that costs: prettier's own form holds two
idents where the author wrote one, so tsv reads it as two — the second is now in name
position and lowercases (`--A\: B` → `--A\: b`), landing on a third form neither formatter
started from. The split is not recoverable, because nothing in the emitted text says the
space was never there.

The same escape blindness in a *value* is [supports_escaped_hash_separator](../supports_escaped_hash_separator_prettier_divergence/)
and [supports_escaped_quote](../supports_escaped_quote_prettier_divergence/); the plain
sibling where both formatters agree on an escaped name is
[media_custom_feature_escaped_name](../media_custom_feature_escaped_name/).

## Reason

See [conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules) — "Escaped separator in a media-feature name".

## Related

- [media_custom_feature_escaped_name](../media_custom_feature_escaped_name/) — an escape elsewhere in the name, where both formatters agree
- [media_feature_case](../media_feature_case/) — the unescaped name-case rule
