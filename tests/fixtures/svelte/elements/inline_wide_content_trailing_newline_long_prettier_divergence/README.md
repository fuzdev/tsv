# inline_wide_content_trailing_newline_long_prettier_divergence

The newline-authored companion of `inline_wide_content_trailing_long`. A wide inline element whose
**own content** (not its attributes) overflows, followed by trailing text the author placed on its
**own line** (a newline boundary). tsv lays the content out **block-style** — both tags intact, the
over-wide content wrapped on its own indented lines to honor printWidth — and keeps the trailing
text on its own line (the newline boundary).

Prettier instead **dangles** the tags and keeps the content on one over-width line.
`unformatted_ours_widecontent.svelte` authors the content on a single line: tsv normalizes it to the
block-style `input` (wrapping within printWidth), while prettier keeps it over-width — so the
**block-style content wrap is the divergence**.

The own-line block-style `input` is dual-stable (tsv and prettier both keep it); the divergence shows
only when the content is authored over-width. (The space-authored counterpart hugs the trailing text
onto the intact `</tag>` — see `inline_wide_content_trailing_long`. Converging the newline boundary
to that hug — reflowing the newline as render-free — is a deliberate, not-yet-converged follow-up.)

## Reason

tsv treats printWidth as a hard limit and lays over-wide inline content out block-style rather than
emitting prettier's single over-width dangled line. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
