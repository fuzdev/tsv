# inline_wide_content_trailing_long_prettier_divergence

A wide inline element whose **own content** (not its attributes) overflows, followed by terminal
trailing text, inside a block (`<p>`). Covers a `<strong>` (no attributes) and an `<a>` (short
`href`) — the divergence is the same for both.

tsv lays the content out **block-style**: both tags stay intact and the over-wide content wraps on
its own indented line(s) so every line stays ≤100. The trailing text **hugs the intact closing tag**
(`</tag> tail`), whichever way its boundary was authored.

Prettier keeps the content on a **single over-width line** and **dangles** the tag delimiters
(`>…content…</tag`) rather than laying out block-style, letting the content exceed printWidth. It
also *distinguishes* the two boundaries, hugging a space and breaking a newline.

```
tsv:       content lays out block-style (≤100), tail hugs `</tag> tail` on either boundary
Prettier:  content on one over-width line,       tail hugs a space, breaks a newline
```

## What each file pins

Both authored boundaries are covered symmetrically, each with the messy authoring tsv normalizes and
the stable form prettier lands on:

| file | authoring | claim |
| --- | --- | --- |
| `unformatted_ours_compact` | everything on one line, space tail | tsv → `input`; prettier → `prettier_variant_compact` |
| `prettier_variant_compact` | prettier's dangled form, `> tail` hugged | prettier keeps it; tsv → `input` |
| `unformatted_ours_multiline` | content on one line, **space** tail | tsv → `input`; prettier does not |
| `unformatted_ours_widecontent` | content on one line, **newline** tail | tsv → `input`; prettier → `prettier_variant_widecontent` |
| `prettier_variant_widecontent` | prettier's dangled form, tail on its own line | prettier keeps it; tsv → `input` |
| `prettier_variant_newline_tail` | `input` exactly, tail on its own line | prettier keeps it; tsv → `input` |
| `variant_blank_line_tail` | `input` with a **blank line** before the tail | **dual-stable** — both formatters keep it |

`unformatted_ours_multiline` and `unformatted_ours_widecontent` are a minimal pair: identical but for
the tail boundary, and both land on `input`. `prettier_variant_newline_tail` isolates that boundary
further still — it differs from `input` in nothing but the newline, so it pins the convergence with
no content reflow confounding it.

## Reason

Two deliberate choices:

1. **Block-style content** — tsv keeps printWidth a hard limit and lays the element out block-style
   (both tags intact, content on its own indented line) rather than emitting prettier's single
   over-width dangled line.
2. **Terminal trailing text hugs, whatever the authored boundary** — the whitespace between a
   closing tag and a *terminal* text sibling is **render-free** under Svelte 5: space and newline
   collapse alike, so it carries no authoring signal to preserve. tsv therefore converges both
   authorings onto the hug rather than reproducing the distinction — one fixed point per document,
   which is what `authoring:audit` grades. This is exactly how a *short* inline element already
   behaves; wide and short elements treat the boundary the same way.

   The rule has two edges, and each is pinned here or next door. An authored **blank line** is a
   Tier-2 signal *independent* of render, so it is not collapsed (`variant_blank_line_tail`; both
   formatters agree, so it is not a divergence). And the scope is *terminal* text — a
   **non-terminal** run, one followed by another flowing element, keeps its own line regardless of
   authoring, because hugging it shifts where that element lands and the result does not converge
   (`inline_wide_content_text_sibling_long`).

See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
