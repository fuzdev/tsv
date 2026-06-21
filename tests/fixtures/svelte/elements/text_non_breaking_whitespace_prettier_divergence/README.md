# text_non_breaking_whitespace_prettier_divergence

How inline text content interacts with break points, and the opening-tag-intact rule:

- **Normal spaces are break points** — a long `<span>` of space-separated words keeps its opening tag
  **intact** and flows the text after `>`, wrapping at spaces (`<span>word1 … word14⏎\tword15 …`).
- **Non-breaking spaces (U+00A0) / narrow NBSP (U+202F) are NOT break points** — text joined by them
  has nowhere to wrap, so the opening tag is **not** attached: the `>` dangles to give the unbreakable
  run its own line, and the run is preserved verbatim.
- Short content stays inline; leading/trailing non-breaking spaces are preserved (not turned into
  regular spaces); root-level regular spaces collapse while non-breaking spaces are kept.

This is the opening-tag-intact rule in action: text attaches its tag **only when it has an internal
break point** to wrap at. Prettier pre-breaks the opening tag uniformly even for breakable text
(`output_prettier.svelte`); tsv attaches it — the divergence. The `unformatted_ours_*` variants
normalize to this form under tsv in one pass.

See [conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
