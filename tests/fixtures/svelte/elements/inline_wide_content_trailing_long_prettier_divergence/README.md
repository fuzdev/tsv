# inline_wide_content_trailing_long_prettier_divergence

A wide inline element whose **own content** (not its attributes) overflows, followed by terminal
trailing text, inside a block (`<p>`). Covers a `<strong>` (no attributes) and an `<a>` (short
`href`) — the divergence is the same for both.

tsv lays the content out **block-style**: both tags stay intact and the over-wide content wraps on
its own indented line(s) so every line stays ≤100. The trailing text's boundary is
**layout-keyed**: a **space** hugs the intact closing tag (`</tag> tail`), and an authored
**newline** keeps the tail's own line — the element renders multiline, so both spellings are clean
and the authored one is kept (dual-stable; the layout-keyed rule). Prettier splits the boundary the
same way, so the divergence here is the **content**: prettier keeps it on a **single over-width
line** and **dangles** the tag delimiters (`>…content…</tag`) rather than laying out block-style.

```
tsv:       content lays out block-style (≤100); tail hugs a space, keeps an authored newline
Prettier:  content on one over-width line;      tail hugs a space, keeps an authored newline
```

## What each file pins

Both authored boundaries are covered symmetrically, each with the messy authoring tsv normalizes and
the stable form prettier lands on:

| file | authoring | claim |
| --- | --- | --- |
| `unformatted_ours_compact` | everything on one line, space tail | tsv → `input`; prettier → `prettier_variant_compact` |
| `prettier_variant_compact` | prettier's dangled form, `> tail` hugged | prettier keeps it; tsv → `input` |
| `unformatted_ours_multiline` | content on one line, **space** tail | tsv → `input`; prettier does not |
| `divergent_variant_widecontent` | prettier's dangled form, tail on its own line | prettier keeps it; tsv rewrites it to `variant_newline_tail`'s form (content block-styled, the newline tail kept) |
| `variant_newline_tail` | `input` exactly, tail on its own line | **dual-stable** — both formatters keep it (the layout-keyed preserve) |
| `variant_blank_line_tail` | `input` with a **blank line** before the tail | **dual-stable** — both formatters keep it |

`variant_newline_tail` isolates the tail boundary with no content reflow confounding it — it
differs from `input` in nothing but the newline, and both formatters hold each form.

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
   formatters agree, so it is not a divergence). And this fixture's scope is *terminal* text — a
   **non-terminal** run, one followed by another flowing element, takes the same per-width answer
   through a different mechanism, the fill's own boundary line rather than the fold
   (`inline_wide_content_text_sibling_long` for prose content,
   `inline_wide_element_content_tail_long` for element-child content) — including an element
   inside a leading inline-sibling wrap, whose joint measurement was retired at the width it
   broke on (`inline_sibling_drop_tail_wide_long`).

See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
