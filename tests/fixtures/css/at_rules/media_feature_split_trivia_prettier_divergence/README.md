# media_feature_split_trivia_prettier_divergence

Where a media-feature expression splits into its **name** and its **value** — everything
before the expression's first colon, and the whole interior when it has none.

`parseMediaFeature` finds that colon with a raw scan whose mode stack tracks nothing tsv
would call trivia, so a colon inside a **comment** or a **string** splits the node just as a
real one does. `(/* : */ 1.50: a)` becomes the name `/*` and the value `*/ 1.50: a`, and
`('x:y' 1.50: a)` the name `'x` and the value `y' 1.50: a` — and because only a
`media-value` takes `adjustNumbers`, the number then normalizes in text the author wrote as
one name.

tsv steps a comment and a string whole, so the colon it finds is one that really separates a
name from a value. The name keeps its number, and the string — still one token — keeps the
quote normalization `adjustStrings` gives it.

`output_prettier.svelte` is what the raw scan costs, in both spellings:

- the comment comes back **re-spelled** (`/* : */` → `/*: */`): its two halves land in
  different nodes and the name half's ` +` collapse runs over the fragment `/*`
- the string comes back **split** (`'x:y'` → `'x: y'`): neither half is a complete string
  any more, so `adjustStrings` finds nothing to normalize and the interior gains a space
  that is content — `'x:y'` and `'x: y'` are different strings

Both of prettier's forms are its own fixed points, so a second pass raises nothing and the
rewrite is silent.

The escaped spelling of the same blindness is
[media_feature_escaped_colon](../media_feature_escaped_colon_prettier_divergence/), where
the payload of a `\:` splits the name; the boundary itself, and the two passes that read it,
are [media_feature_name_position](../media_feature_name_position/).

## Reason

Content preservation. See
[conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
— "Media feature split trivia", and **The two prelude readers** for the boundary.

## Related

- [media_feature_name_position](../media_feature_name_position/) — the split itself, where the two agree
- [media_feature_escaped_colon](../media_feature_escaped_colon_prettier_divergence/) — the escaped spelling
