# import_url_string_comment_prettier_divergence

A comment **trailing** the quoted argument of `@import`'s `url()`.

tsv: `@import url('a.css' /* c */);` (normalized single spaces)
Prettier: `@import url('a.css'/* c */);` (authored spacing frozen — and it closes the
boundary up even when the author spaced, so the two forms never converge)

A run of two comments takes the same rule (`/* c1 */ /* c2 */`, joined single-spaced,
where prettier glues) — the value-position rule of
[comma_comment_glued_run](../../values/lists/comma_comment_glued_run_prettier_divergence/)
reached from the other authoring. `unformatted_ours_glued` pins that both authorings
converge under tsv; `unformatted_ours_double_quotes` pins that the quote still normalizes
with a comment present (`"a.css"` → `'a.css'`), which is the point of the argument keeping
the structured string path rather than falling through to the opaque bare-URL one.

Spacing is safe in this position: a comment yields no token (CSS Syntax 3 §4.3.2
`consume comments` **returns nothing**) and the argument is a single `<string>` either
way, so both forms tokenize identically. Contrast a selector compound, where a space is
structure and a comment run therefore stays glued (see
[combinator_comment](../../selectors/combinator_comment_svelte_prettier_divergence/)).

## Scope: only a *trailing* comment reaches this rule

A comment **leading** the argument (`url(/* c */ 'a.css')`) is a different token entirely.
CSS Syntax 3 §4.3.4 "consume an ident-like token" decides url-token vs function-token on
whether `url(` is followed by a quote — optionally past *whitespace*, and a comment is not
whitespace there (`consume comments` runs when a token *starts*, not inside one). So a
leading comment makes the whole thing a **url-token**, whose contents are raw text: tsv
emits it verbatim (author's spacing kept, quote not normalized), which is why it is absent
from this fixture. Prettier normalizes that position anyway, so it diverges too — but by
tsv preserving rather than by tsv normalizing, and preserving a url-token's contents is the
spec-side answer.

## Reason

Stable quirk. tsv normalizes comment spacing consistently across all CSS contexts. See
[conformance_prettier_css.md §CSS: Comments](../../../../../docs/conformance_prettier_css.md#css-comments).

## Related

- [atrule_in_prelude](../../tokens/comments/atrule_in_prelude_prettier_divergence/) — the same rule mid-prelude
- [import_layer_name_comment](../import_layer_name_comment/) — the sibling `layer()` argument, where tsv and prettier agree
- [import_url_comment_chars](../import_url_comment_chars/) — `/*` inside an unquoted URL, where it is url content and not a comment at all
