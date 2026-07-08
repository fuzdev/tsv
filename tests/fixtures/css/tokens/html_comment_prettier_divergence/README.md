# html_comment_prettier_divergence

Legacy `<!-- … -->` HTML-comment markers in CSS — the CDO/CDC tokens from the old
`<style><!-- … --></style>` browser-hiding idiom.

Svelte's `parseCss` reads `<!--` … `-->` as one span and **discards everything between the
markers** (like a block comment) at any stylesheet whitespace/comment boundary — top-level,
between rules, before a declaration — emitting **no** AST node. tsv is a drop-in for `parseCss`,
so it matches that: the CDO/CDC span drops on format.

tsv: `<!-- p { color: blue } -->` → dropped entirely (matches `parseCss`)
Prettier: `<!-- p { color: blue } -->` → `<!-- p { … } -- >` — mangles the markers into invalid
CSS but keeps the inner `color: blue`

Prettier emits invalid output rather than throwing, so there is no prettier oracle here — hence the
divergence. Svelte's swallow itself departs from the CSS Syntax spec, where `<!--` (CDO) and `-->`
(CDC) are two *independent* no-op tokens and the content between them parses as ordinary CSS (so the
spec — and, mangling aside, prettier — keep the `p` rule; `parseCss`, and therefore tsv, drop it).
tsv inherits `parseCss` behavior by design, not the spec's token rules — the parse side (which
positions swallow vs. keep the markers raw) is cataloged as a Svelte compat behavior in
[conformance_svelte.md §CSS Compat Behaviors](../../../../../docs/conformance_svelte.md#css-compat-behaviors).

## Variants

- `unformatted_ours_top_level` — empty `<!-- -->` before a rule; tsv drops it. Degenerate case:
  nothing lives between the markers, so spec and `parseCss` agree and no content is lost.
- `unformatted_ours_wraps_rule` — `<!-- p { color: blue } -->` around a real rule; tsv drops the
  whole span, so `p { color: blue }` vanishes. The case where the swallow is actually observable.

## Reason

See [conformance_prettier.md §CSS: HTML comments (CDO/CDC)](../../../../../docs/conformance_prettier.md#css-html-comments-cdocdc).
Prettier emits invalid CSS on the CDO/CDC markers (no oracle); tsv follows Svelte's `parseCss`,
which swallows the `<!-- … -->` span.

## Related

- Parse-parity (tsv AST == `parseCss` for the CDO/CDC forms, which format-drop can't capture) is
  pinned by `tests/css_cdo_cdc.rs` and the svelte-fixtures conformance gate (`css/samples/comment-html`).
