# Divergence: an own-line block comment in a pattern's bracket→`:` gap collapses

A block comment the author put on its own line between a destructuring pattern's closing
`}`/`]` and its `:` annotation. tsv collapses the unforced breaks and keeps the comment
**in its authored gap**, inline; prettier expands the pattern and relocates the comment
*into the brackets*, trailing the last element.

```ts
// authored                // tsv (collapses in place)   // prettier (into the brackets)
const { a }                const { a } /* c */ : T = x;  const {
/* c */                                                    a
: T = x;                                                   /* c */
                                                         }: T = x;
```

Own-line-ness is authoring signal for a *leading* position, not a trailing one: a
single-line block trailing a head token is pure layout there, so tsv collapses its break
while preserving the side of the separator the author chose — which is the association
prettier's relocation destroys. A run keeps its order with each comment distinct.

The inline authoring of this same gap is a **match** —
[pattern_bracket_colon_comment](../pattern_bracket_colon_comment/) — so prettier is a
fixed point on `input.svelte` and there is no `output_prettier.*`; the divergence is
entirely in what each formatter does with the own-line *authoring*. Prettier's landing is
dual-stable (`variant_own_line.svelte`): once the comment sits **inside** the brackets it
is an authored position of its own, and tsv preserves it there rather than pulling it back
out — the two forms record two different associations, which is the point.

The pattern spelling of
[binding_key_colon_own_line_block_comment](../../../declarations/variable/binding_key_colon_own_line_block_comment_prettier_divergence/),
and the sibling of
[rename_key_colon_own_line_block_comment](../rename_key_colon_own_line_block_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
