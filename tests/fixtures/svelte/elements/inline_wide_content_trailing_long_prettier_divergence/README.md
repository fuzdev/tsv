# inline_wide_content_trailing_long_prettier_divergence

A wide inline element whose **own content** (not its attributes) overflows, followed by
trailing text authored with a **space** boundary, inside a block (`<p>`). Covers a `<strong>`
(no attributes) and an `<a>` (short `href`) — the divergence is the same for both. The
newline-authored counterpart is the sibling `inline_wide_content_trailing_newline_long`.

tsv treats printWidth as a hard limit: the over-wide content wraps **inside** the element so every
line stays ≤100, and the trailing text **hugs** the dangled closing `>` (`</tag⏎> tail`), respecting
the author's space. The no-attribute `<strong>` keeps its **opening tag attached** (`<strong>word…`,
text flows after `>`); the attributed `<a>` still dangles its opening `>` for now — the with-attrs
opening-attach is a pending follow-up (attrs-wrap makes the naive form non-idempotent). The two
`unformatted_ours_*` variants pin idempotence: the single-line `unformatted_ours_compact` and the
multiline-authored `unformatted_ours_multiline` both normalize to `input.svelte` in one pass.

Prettier keeps the content on a **single over-width line** (`>…content…</tag`) — it lets the
content exceed printWidth rather than wrap it — but hugs the trailing text the same way
(`output_prettier.svelte`), so the **sole divergence is the content wrap**.

```
tsv:       content wraps across lines (≤100), trailing text hugs `> tail`
Prettier:  content on one line (>100),        trailing text hugs `> tail`
```

## Reason

Two deliberate choices:

1. **Content wraps** — tsv keeps printWidth a hard limit and wraps the element's content rather
   than emitting prettier's single over-width line. (The content wrap is the only difference from
   prettier here.)
2. **Trailing text hugs the dangled `>`** — tsv treats the boundary whitespace before trailing
   text as a **meaningful authoring choice**, not noise to normalize away. A *space* (`</tag> tail`)
   means "keep the text attached," so tsv flows it onto the dangled-`>` line; a *newline*
   (`</tag>⏎tail`, the sibling fixture) means "put it below," so tsv keeps it on its own line. This
   is exactly how a *short* inline element already behaves (`<el>x</el> tail` for a space,
   own-line for a newline) — wide and short elements now treat the boundary the same way.

   This deliberately makes the wide case authoring-*dependent* (space ≠ newline), the same as the
   short case and the same as Prettier here (Prettier hugs a space, breaks a newline). tsv is *not*
   trying to be more authoring-independent than the author's intent warrants: where both authorings
   are clean and legible, the author's distinct intent is preserved. (Forcing convergence is
   reserved for cases where one authoring would be degenerate — over-width, a split tag, or an
   unstable form — e.g. the non-terminal cascade in `inline_wide_content_text_sibling_long`.)

See [conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
