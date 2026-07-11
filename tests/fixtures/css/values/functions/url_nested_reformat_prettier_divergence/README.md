# url_nested_reformat_prettier_divergence

An unquoted `url()` whose content contains a **nested `(...)`** group
(`url(a(b,c))`, `url(a( b ))`). Per CSS Syntax 3 §4.3.6 an unquoted `url(...)` is
consumed as one opaque `<url-token>` — its content is *not* re-parsed as
component values — so tsv preserves it **verbatim** and formats each of these to
itself.

Prettier instead treats the nested group as a value and reformats inside it:

- `url(a(b,c))` → `url(a(b, c))` — a space is added after the comma
- `url(a( b ))` → `url(a(b))` — the interior whitespace is dropped

tsv is the more defensible side: the url content is opaque per the token grammar,
so reformatting inside it is a divergence, not a normalization. Both formatters
keep their own output stable (idempotent).

`output_prettier.svelte` is prettier's reformatted output; `input.svelte` is
tsv's verbatim form. Surfaced on prettier's own corpus by
`tests/format/css/inline-url/inline_url.css` (`url(--var(foo-bar,#dadce0))` →
`url(--var(foo-bar, #dadce0))`) and `tests/format/css/loose/loose.css`
(`url(var( x ))` → `url(var(x))`).

See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values).
