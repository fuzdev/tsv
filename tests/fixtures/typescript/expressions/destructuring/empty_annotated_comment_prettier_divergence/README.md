# Divergence: an ANNOTATED empty pattern's interior comment is hoisted out

An empty destructuring pattern whose only body content is an inline block comment, where
the pattern also carries a `: Type` annotation. tsv keeps the comment inside the brackets
the author wrote it in; prettier **hoists it out** of them, into the pattern's
bracket→`:` gap.

```ts
// tsv (stays inside)          // prettier (hoisted out)
const { /* c1 */ }: T = x;     const {} /* c1 */ : T = x;
const [/* c2 */]: U = y;       const [] /* c2 */ : U = y;
```

The annotation is what triggers it: with no annotation prettier leaves the comment inside
and the two formatters differ only by the bracket padding — the separate
[empty_comment](../empty_comment_prettier_divergence/) divergence for `{}`, and a plain
match for `[/* c */]`. So this is a **relocation**, not a spacing choice: the comment
moves from inside the brackets to outside them, changing what it reads as being about,
and it is the position tsv preserves by default.

Prettier lands the comment in the same gap the pattern-head entries cover — the
bracket→`:` gap, where a comment the author wrote *outside* stays outside in both
([pattern_bracket_colon_comment](../pattern_bracket_colon_comment/)). Both destinations
being the same slot is what makes the hoist lossy in principle: an author who wrote one
comment inside and one outside gets them merged into one run, in one position.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
