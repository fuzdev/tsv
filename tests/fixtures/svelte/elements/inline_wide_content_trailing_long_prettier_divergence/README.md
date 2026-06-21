# inline_wide_content_trailing_long_prettier_divergence

A wide inline element whose **own content** (not its attributes) overflows, followed by
trailing text inside a block (`<p>`). Covers a `<strong>` (no attributes) and an `<a>`
(short `href`) — the divergence is the same for both.

tsv treats printWidth as a hard limit: the over-wide content wraps **inside** the element
(`<tag⏎\t>word word …⏎\tword</tag⏎\t>`) and the trailing text takes its **own line** after the
dangled closing `>`. Every line stays ≤100. tsv **never hugs** trailing text onto a wrapped
element's last line — uniformly, regardless of how the source was authored (the two
`unformatted_ours_*` variants pin that idempotence: the single-line `unformatted_ours_compact`
and the multiline-authored `unformatted_ours_multiline` both normalize to `input.svelte` in one
pass).

Prettier keeps the content on a **single over-width line** (`>…content…</tag`) — it lets the
content exceed printWidth rather than wrap it. For *this* input it places the trailing text on
its own line too (`output_prettier.svelte`), so the **sole divergence is the content wrap**.

```
tsv:       content wraps across lines (≤100), trailing text on its own line
Prettier:  content on one line (>100),        trailing text on its own line
```

Note: prettier's trailing-text placement is **authoring-dependent** — fed a compact
`…</tag> tail` source it hugs `> tail`, fed `…</tag>⏎tail` it breaks — whereas tsv's never-hug is
authoring-independent (one canonical form). That uniformity is the point: it lets the fill
renderer drop its position-aware "hug after a break" special case, so one rule governs every fill
(inline content and CSS value lists alike).

## Reason

Two deliberate choices, each trading proximity-to-prettier for tsv's own consistency:

1. **Content wraps** — tsv keeps printWidth a hard limit and wraps the element's content rather
   than emitting prettier's single over-width line.
2. **Trailing text on its own line** — when the element wraps and its closing `>` dangles at a low
   column, tsv does **not** reflow the trailing text onto that line, even though it would fit
   there. One uniform rule — "break the separator after a fill item that wrapped at line start" —
   governs every fill (inline content and CSS value lists alike), with no position-aware special
   case. **The tradeoff**: a little unused width on the dangling-`>` line, and a mild inconsistency
   with a _short_ inline element (which keeps its following text inline, `<el>x</el> tail`, because
   it never wraps). **The gain**: a single fill rule and authoring-independent (idempotent) output —
   the formatter can't be coaxed into two different shapes by how the source was wrapped.

See [conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
