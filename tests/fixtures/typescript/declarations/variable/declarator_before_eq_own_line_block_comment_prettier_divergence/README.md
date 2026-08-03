# declarator_before_eq_own_line_block_comment_prettier_divergence

An own-line **block** comment in a variable declarator's name→`=` gap
(`const a⏎/* c */⏎= 1;`). A single-line block forces nothing, and a comment in
this gap trails the name (a trailing position), so tsv collapses the authored
breaks and keeps the comment inline in its authored syntactic slot
(`const a /* c */ = 1;` — the form both formatters hold stable when authored
inline). Prettier instead **relocates** the comment across the `=` and hangs it
leading the value (`const a =⏎\t/* c */⏎\t1;`).

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier takes it to the relocated hang instead (in one pass — no
  intermediate).
- `variant_own_line.svelte` — prettier's landing form, dual-stable: there the
  comment sits *after* the `=`, leading the value — a different syntactic
  position, which both formatters preserve (the value-gap own-line rule). An
  author who wants the comment leading the value writes it there; tsv honors
  both positions and collapses only the line structure of the trailing one.

A run of blocks collapses in order, each comment kept distinct — lossless. The
same-gap **line** comment (which forces the break) is the sibling
[declarator_before_eq_line_comment](../declarator_before_eq_line_comment_prettier_divergence/);
the class property takes the same outcome
([property_before_eq_own_line_block_comment](../../class/property_before_eq_own_line_block_comment_prettier_divergence/)),
while the enum member's chain converges back to the inline form
([member_before_eq_own_line_block_comment](../../enum/member_before_eq_own_line_block_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
