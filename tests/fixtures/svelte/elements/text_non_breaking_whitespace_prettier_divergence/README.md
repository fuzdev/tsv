# text_non_breaking_whitespace_prettier_divergence

How inline text content interacts with break points under block-style layout. tsv lays an inline
element's wrapping text content out **block-style** (both tags intact, content on its own indented
line); whether that content then *wraps* depends on its break points:

- **Normal spaces are break points** — a long `<span>` of space-separated words lays out block-style
  and the content fill-wraps across lines (`word1 … word15⏎\tword16 …`).
- **Non-breaking spaces (U+00A0) / narrow NBSP (U+202F) are NOT break points** — text joined by them
  has nowhere to wrap, so the content stays on a single line within the block-style body and the run
  is preserved verbatim.
- Short content stays inline; leading/trailing non-breaking spaces are preserved (not turned into
  regular spaces); root-level regular spaces collapse while non-breaking spaces are kept.

Prettier instead **dangles** the tag delimiters (pre-breaking the opening tag) even for breakable
text; tsv keeps both tags intact and lays out block-style — the divergence. The `unformatted_ours_*`
variants normalize to this form under tsv in one pass.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
