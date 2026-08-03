# computed_key_bracket_own_line_block_comment_prettier_divergence

An own-line **block** comment in a class computed property key's `]`→`=` gap
(`[y]⏎/* c */⏎= 2;`). A single-line block forces nothing, and a comment in this
gap trails the `]` (a trailing position), so tsv collapses the authored breaks
and keeps the comment inline in its authored syntactic slot
(`[y] /* c */ = 2;` — the spaced inline form, input, matching the before-`=`
family's parity).

Prettier has **two destinations from this one gap, keyed on the authoring**:

- From the inline authoring (input) it relocates the comment **into the
  brackets** (`[y /* c */] = 2;` — `output_prettier.svelte`), the
  already-cataloged after-`]` move its same-line sibling
  [computed_key_bracket_comment](../computed_key_bracket_comment_prettier_divergence/)
  pins across all member kinds.
- From the **own-line** authoring it instead crosses the `=` and hangs the
  comment leading the value (`[y] =⏎\t\t/* c */⏎\t\t2;` —
  `variant_own_line.svelte`, dual-stable in both formatters; one pass, no
  intermediate) — the same hang the plain class-property sibling
  [property_before_eq_own_line_block_comment](../../../declarations/class/property_before_eq_own_line_block_comment_prettier_divergence/)
  lands on. `unformatted_ours_own_line.svelte` pins tsv's side: the own-line
  authoring normalizes to input in one pass.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
