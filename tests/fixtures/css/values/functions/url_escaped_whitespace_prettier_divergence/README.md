# url_escaped_whitespace_prettier_divergence

An unquoted `url(...)` whose content **ends** in an escaped whitespace (`url(x\ )`).

An unquoted url is an opaque `<url-token>`, but its tokenizer still consumes escapes
(css-syntax-3 §4.3.6 defers to §4.3.7): `\ ` is a valid escape whose escaped code
point **is that space**, so `url(x\ )` is the url `x ` — the space is *content*, and
the `)` that follows it closes the token.

tsv preserves it, so `input.svelte` formats to itself.

**Prettier trims it as if it were padding**, stranding the backslash onto the closing
paren:

| source | prettier | consequence |
| --- | --- | --- |
| `border-image: url(a\ b);` | `url(a\ b)` | correct — the escape is *inside* the content, so nothing abuts it |
| `background: url(x\ );` | `url(x\)` | `\)` escapes the closer — the url token never terminates |

The second makes prettier's own output **fail to re-parse**: tsv's CSS parser rejects
`output_prettier.svelte` with `Expected '}'`, because the url token swallows the rest
of the stylesheet looking for a closing paren.

tsv declines to reproduce it: **its format→re-parse invariant outranks matching
prettier.** Emitting output that does not parse is never the defensible side, whatever
the reference formatter does.

This is the `url()` face of the same rule cataloged for ordinary values — an escape is
opaque, and its payload is content, not padding. See
[css/values/escaped_whitespace](../../escaped_whitespace_prettier_divergence/) for the
declaration/function/selector faces, and
[url_escaped_paren_ws](../url_escaped_paren_ws_prettier_divergence/) for the *outer*
whitespace inside the parens (which tsv does trim, per §4.3.6).

See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values).
