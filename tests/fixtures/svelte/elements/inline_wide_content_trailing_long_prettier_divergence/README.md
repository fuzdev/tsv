# inline_wide_content_trailing_long_prettier_divergence

A wide inline element whose **own content** (not its attributes) overflows, followed by
trailing text authored with a **space** boundary, inside a block (`<p>`). Covers a `<strong>`
(no attributes) and an `<a>` (short `href`) — the divergence is the same for both. The
newline-authored counterpart is the sibling `inline_wide_content_trailing_newline_long`.

tsv lays the content out **block-style**: both tags stay intact and the over-wide content wraps on
its own indented line(s) so every line stays ≤100. The space-authored trailing text **hugs the
intact closing tag** (`</tag> tail`), respecting the author's space; a newline boundary instead
keeps the text on its own line (the sibling fixture). The two `unformatted_ours_*` variants pin
idempotence: the single-line `unformatted_ours_compact` and the multiline-authored
`unformatted_ours_multiline` both normalize to `input.svelte` in one pass.

Prettier keeps the content on a **single over-width line** and **dangles** the tag delimiters
(`>…content…</tag`) rather than laying out block-style, letting the content exceed printWidth; it
hugs the trailing text the same way. So the **sole divergence is the block-style content layout**.

```
tsv:       content lays out block-style (≤100), trailing space hugs `</tag> tail`
Prettier:  content on one over-width line,       trailing space hugs the dangled `> tail`
```

## Reason

Two deliberate choices:

1. **Block-style content** — tsv keeps printWidth a hard limit and lays the element out block-style
   (both tags intact, content on its own indented line) rather than emitting prettier's single
   over-width dangled line.
2. **Trailing text follows the authored boundary** — tsv treats the boundary whitespace before
   trailing text as a **meaningful authoring choice**, not noise to normalize away. A *space*
   (`</tag> tail`) means "keep the text attached," so tsv hugs it onto the closing-tag line; a
   *newline* (`</tag>⏎tail`, the sibling fixture) means "put it below," so tsv keeps it on its own
   line. This is exactly how a *short* inline element already behaves — wide and short elements treat
   the boundary the same way.

   This deliberately makes the wide case authoring-*dependent* (space ≠ newline), the same as the
   short case and the same as Prettier here (Prettier hugs a space, breaks a newline). tsv is *not*
   trying to be more authoring-independent than the author's intent warrants: where both authorings
   are clean and legible, the author's distinct intent is preserved. (Forcing convergence is
   reserved for cases where one authoring would be degenerate — over-width, a split tag, or an
   unstable form — e.g. the non-terminal cascade in `inline_wide_content_text_sibling_long`.)

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
