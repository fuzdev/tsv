# attributes_key_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in an import attribute's key→`:` gap
(`with { type⏎/* c */⏎: 'json' }`). A single-line block forces nothing, and a
comment in this gap trails the key (a trailing position), so tsv collapses the
authored breaks and keeps the comment inline in its authored syntactic slot
(`type /* c */: 'json'` — the form both formatters hold stable when authored
inline). Prettier instead **relocates** the comment across the `:` and hangs it
leading the value (`type:⏎\t/* c */⏎\t'json'`).

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier takes it to the relocated hang instead.
- `variant_own_line.svelte` — prettier's landing form, dual-stable: there the
  comment sits *after* the `:`, leading the value — a different syntactic
  position, which both formatters preserve (the value-gap own-line rule the
  sibling
  [attributes_value_colon_line_comment](../attributes_value_colon_line_comment_prettier_divergence/)
  documents for the line-comment case).

The same-gap **line** comment (which forces the break) is
[attributes_key_colon_line_comment](../attributes_key_colon_line_comment_prettier_divergence/);
the inline-authored block is a plain match in both formatters
([attributes_comma_comment](../attributes_comma_comment/)).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
