# url_escaped_paren_ws_prettier_divergence

An unquoted `url()` whose content contains an **escaped paren** (`\(`) and carries
leading/trailing whitespace inside the outer parens.

Per CSS Syntax 3 §4.3.6 an unquoted `url(...)` is one opaque `<url-token>` whose
tokenizer **consumes the leading and trailing whitespace** — so the value is trimmed.
tsv follows this: `url(  a\(b  )` → `url(a\(b)`, so `prettier_variant_outer_ws`
normalizes to `input`.

Prettier's CSS parser (postcss) mishandles the escaped paren — the same bug behind
[url_escaped_paren](../url_escaped_paren_prettier_divergence/), where `url(a\)b)` makes
it throw `Unbalanced parenthesis`. Here it doesn't throw, but its escaped-paren path
**stops normalizing** the whitespace and keeps it verbatim: `url(  a\(b  )` stays
`url(  a\(b  )`. On plain content (no escaped paren) prettier trims exactly like tsv, so
the divergence is escaped-paren-only.

No `output_prettier.*`: on tsv's canonical trimmed form (`input`) prettier agrees, so the
divergence lives only in `prettier_variant_outer_ws` — a form prettier keeps stable that
tsv normalizes to `input`. Surfaced on Prettier's own corpus by
`tests/format/css/url/url.css` (its `content: url(  …\(\(.jpg  )` lines).

See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values).
