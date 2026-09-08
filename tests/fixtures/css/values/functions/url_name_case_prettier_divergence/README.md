# url_name_case_prettier_divergence

An unquoted `url()` spelled in any case but lowercase (`URL(x ,y)`, `Url(a,.50)`).

Per CSS Syntax 3 §4.3.4 ("Consume an ident-like token") an ident that is an **ASCII
case-insensitive** match for `url` followed by `(` opens a `<url-token>`, whose content is
opaque (§4.3.6): `URL(x ,y)` is the url `x ,y`, exactly as `url(x ,y)` is. tsv's lexer
reads it that way, and so does the text normalizer that re-reads a value from source (a
declaration carrying a comment, an `@import` prelude's tail): the token is kept verbatim
and only the padding inside its parens trims (`URL( x ,y )` → `URL(x ,y)`), the same
treatment [url_comma_content](../url_comma_content/) pins for the lowercase spelling.

Prettier's value parser exempts a url-token from function parsing only when the name is
the exact lowercase word `url`, so `URL(` and `Url(` are read as ordinary **functions**
and their argument lists normalize: the comma takes a space and the number its leading
zero — `URL(x, y)`, `Url(a, 0.5)` — which changes the URL itself (`x ,y` and `x, y` are
two different resources). The same case-sensitive compare is what makes `URL(//x)` freeze
where `url(//x)` is opaque — see
[line_comment_arg](../line_comment_arg_prettier_divergence/).

`output_prettier.svelte` carries prettier's form. `unformatted_ours_padding` pads the
parens: tsv trims it to `input`, prettier to its own form.

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values).
