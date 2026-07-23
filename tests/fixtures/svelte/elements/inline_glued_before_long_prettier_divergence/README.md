# inline_glued_before_long_prettier_divergence

Two glued-before shapes where tsv does **not** dangle the closing `>` — the cases the
sibling-`>` dangle-onto-text (`inline_glued_both_dangle_long`) deliberately excludes. That
dangle fires only while the dangle line (`prefix<span>content</span`) fits ≤100 and there is
no trailing whitespace to wrap at; outside those bounds tsv resolves the over-width line with
block-style or a trailing-space wrap, where prettier dangles or overflows.

- **Dangle line over 100 → block-style.** When the glued prefix (or the content) is wide
  enough that `prefix<span>content</span` would itself exceed printWidth, dangling the `>`
  wouldn't help, so tsv lays the element out block-style (opening tag ends the prefix line,
  content on its own indented line, closing tag begins the suffix line). This case sits one
  character past the widest prefix that still dangles — the dangle@100 boundary is the fourth
  case of [inline_glued_both_dangle_long](../inline_glued_both_dangle_long/). Prettier
  **double-dangles** (`<span⏎>x</span⏎>`), pre-breaking the opening tag to hold the content
  against the dangled delimiters.
- **Trailing space → wrap.** Glued before but space-separated after, tsv wraps at that
  trailing whitespace, giving two lines that each fit. Prettier keeps the whole thing on one
  line and lets it run past printWidth.

Cases (in order): dangle-line@101 (block-style); glued-before+space fits@100 (inline,
control); glued-before+space@101 (wrap at the trailing space).

Both boundaries tsv moves are render-free under Svelte 5 — the block-style content boundary
is trimmed at compile, and the trailing-space wrap turns inter-node whitespace (which
collapses to one space) into a line break — so the output parses to a byte-identical AST
(confirmed by `ast_diff --render`).

tsv: block-style or a trailing-space wrap, never a dangle here.
`prettier_variant_kept.svelte` is prettier's stable form — double-dangle (block-style case)
and the kept over-width line (space case) — which tsv normalizes back to `input.svelte`.
`unformatted_ours_compact.svelte` is the compact one-line authoring both formatters start
from: tsv normalizes it to `input.svelte`, prettier to `prettier_variant_kept.svelte`.

## Reason

Design choice, render-free under Svelte 5. The closing-`>` dangle onto glued following text
(the sibling-`>` dangle generalized) applies only while it fits and there is no trailing
space; past that bound tsv falls back to block-style (width-optimal — content on its own
line handles any width) or a trailing-space wrap, rather than prettier's double-dangle or
over-width line.
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
