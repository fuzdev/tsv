# computed_key_bracket_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in a destructuring computed key's `]`→`:` gap
(`{ [x]⏎/* c */⏎: a }`, also in a parameter pattern). A single-line block
forces nothing, and a comment in this gap trails the `]` (a trailing
position), so tsv collapses the authored breaks and keeps the comment inline
in its authored syntactic slot — glued, `{ [x] /* c */: a }`, the same
per-site parity as its inline sibling
([computed_key_bracket_comment](../computed_key_bracket_comment_prettier_divergence/)).

Prettier has **two destinations from this one gap, keyed on the authoring**:

- From the inline authoring (input) it relocates the comment **into the
  brackets** (`{ [x /* c */]: a }` — `output_prettier.svelte`), the
  already-cataloged after-`]` move its inline sibling pins.
- From the **own-line** authoring it instead crosses the `:`, expanding the
  pattern and hanging the comment leading the local name
  (`{⏎\t[x]:⏎\t\t/* c */⏎\t\ta⏎}` — `divergent_variant_own_line.svelte`, one
  pass, prettier-stable). That landing is **not** tsv-stable — a destructuring
  pattern has no multiline preservation, so tsv re-collapses prettier's form,
  keeping the comment in its relocated after-`:` slot, glued leading the local
  (`{ [x]: /* c */ a }`, a third stable form). Three stable forms coexist,
  keyed on which side of the `:` the comment was authored; only prettier moves
  a comment between them.

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes
  it to input; prettier takes it to the relocated hang instead (one pass — no
  intermediate).
- `divergent_variant_own_line.svelte` — prettier's landing; tsv rewrites it to
  the third form above.

The computed-key face of the rename sibling
([rename_key_colon_own_line_block_comment](../rename_key_colon_own_line_block_comment_prettier_divergence/));
the same-gap **line** comment is
[computed_key_bracket_colon_line_comment](../computed_key_bracket_colon_line_comment_prettier_divergence/);
the iface/type-literal host converges on the in-bracket relocation instead
([computed_key_bracket_colon_own_line_block_comment](../../../types/type_members/computed_key_bracket_colon_own_line_block_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
