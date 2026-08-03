# computed_key_bracket_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in a computed property key's `]`→`:` gap
(`{ [x]⏎/* c */⏎: 1 }`). A single-line block forces nothing, and a comment in
this gap trails the `]` (a trailing position), so tsv collapses the authored
breaks and keeps the comment inline in its authored syntactic slot
(`[x] /* c */: 1` — the glued inline form, input; the object stays expanded
because tsv preserves an object literal's authored multiline-ness).

Prettier has **two destinations from this one gap, keyed on the authoring**:

- From the inline authoring (input) it relocates the comment **into the
  brackets** (`[x /* c */]: 1` — `output_prettier.svelte`), the already-cataloged
  after-`]` move its same-line sibling
  [computed_key_bracket_colon_comment](../computed_key_bracket_colon_comment_prettier_divergence/)
  pins across all member kinds.
- From the **own-line** authoring it instead crosses the `:` and hangs the
  comment leading the value (`[x]:⏎\t\t/* c */⏎\t\t1` —
  `variant_own_line.svelte`, dual-stable in both formatters; one pass, no
  intermediate). `unformatted_ours_own_line.svelte` pins tsv's side: the
  own-line authoring normalizes to input in one pass.

The class-property `]`→`=` sibling is
[computed_key_bracket_own_line_block_comment](../../../statements/class/computed_key_bracket_own_line_block_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
