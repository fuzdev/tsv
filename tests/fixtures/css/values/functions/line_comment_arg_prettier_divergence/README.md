# line_comment_arg_prettier_divergence

A function whose argument list contains a `//` (`fn(//x, 0.1)`).

CSS has no line comments: per CSS Syntax 3 §4.3.1 a `/` not followed by `*` is a
`<delim-token>`, so `//` is two delimiters and ordinary content of the argument list.
tsv reads it as such, and the list normalizes around it like any other — the separator
takes its space and the number its leading zero (`fn(//x,.10)` → `fn(//x, 0.1)`) — in a
declaration value, a `@supports` condition and an `@import` prelude alike, wherever the
value reader runs. Only an unquoted `url()` is different, and there both formatters agree:
its content is one opaque `<url-token>` (§4.3.6), so `url(//a.fuz.dev/x)` is kept whole —
in the lowercase spelling; prettier's exemption is case-sensitive, so `URL(` freezes like
any function (see [url_name_case](../url_name_case_prettier_divergence/)).

Prettier's value parser runs postcss-values-parser in `loose` mode, where a `//` outside a
`url(` starts a **line comment** that runs to the end of the line — an SCSS/LESS spelling
applied to CSS. The comment swallows the function's closing `)`, the parse throws on the
unbalanced paren, and prettier falls back to emitting the whole value **verbatim** — the
same freeze, one construct over, as
[escaped_paren_arg](../escaped_paren_arg_prettier_divergence/). So every authoring of the
value is a prettier fixed point, `.10`, glued comma and (in a prelude) the author's line
break included; `prettier_variant_newline` carries the prelude shape prettier's own suite
spells (`css/no-semicolon/url.css`, whose `ur⏎ l(//…)` is a `url(` split in two).

No `output_prettier.*`: on tsv's canonical form (`input`) prettier agrees, so the
divergence lives only in the `prettier_variant_*` forms — forms prettier keeps stable that
tsv normalizes to `input`.

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values).
