# attributes_comma_gap_comment_prettier_divergence

A **block** comment in an import attribute list's comma gap that shares a line
with something — the comma itself, the previous attribute, or another comment.
Such a comment does not own its line, so it forces nothing: the `with { … }`
clause stays inline and the comment keeps the side of the comma the author chose.

- `unformatted_ours_comma_own_line.svelte` — the comma pushed onto its own line
  with the comment behind it (`'json'⏎, /* c */⏎attr:`). tsv normalizes it to
  input, the comment still after the comma; prettier slides it **backward** across
  the comma and lands on `variant_comment_before_comma`.
- `unformatted_ours_after_comma.svelte` — the same comment ending the comma's
  line, the next attribute below it. Same two answers.
- `unformatted_comma_glued.svelte` — the comment starting a line with the comma
  glued behind it (`'json'⏎/* c */,⏎attr:`). It is adjacent to the comma, so
  **both** formatters collapse it inline: a plain `unformatted_*`.
- `variant_comment_before_comma.svelte` — prettier's landing form, dual-stable.
  Both sides of the comma are stable in both formatters, which is why tsv
  preserves the authored one rather than canonicalizing.

An **isolated** block — a line of its own on both sides (`'json',⏎/* c */⏎attr:`)
— still expands the clause in both formatters, as does any line comment; those
are the plain matches in
[attributes_comma_comment](../attributes_comma_comment/).

## Reason

Sliding a comment backward across the previous item's own comma is the move tsv
refuses ([comments.md](../../../../../../docs/comments.md) §The element-comma
seam): the comma is re-emitted structure, so a comment written after it belongs
after it. Own-line-ness is read from the **source** — a comment owns its line
only when a newline both precedes and follows it, the condition prettier's own
`printLeadingComment` uses to emit a hardline — and an attribute clause flattens
when it fits, so the author's break around the comma is layout
([conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
