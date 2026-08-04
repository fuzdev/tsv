# optional_marker_own_line_block_comment_prettier_divergence

An own-line **block** comment in the `?`→`:` marker gap of a property
signature, type-literal member, or class property (`a?⏎/* c */⏎: number;`). A
single-line block forces nothing, and a comment in this gap trails the marker
(a trailing position), so tsv collapses the authored breaks and keeps the
comment inline in its authored syntactic slot — the spaced form
`a? /* c1 */ : number;` — in one pass.

Prettier's side is the **already-cataloged** after-`?` relocation, not a new
move: from the inline authoring it relocates the interface and type-literal
comments to *before* the `?` (`a /* c1 */?: number;` — `output_prettier.svelte`;
see [optional_marker_comment](../../../types/type_literal/optional_marker_comment_prettier_divergence/)
and the interface arm
[modifier_after_comment](../../../types/type_members/modifier_after_comment_prettier_divergence/)),
while keeping the class property inline (a match pinned in
[property_modifier_type_comment](../../../statements/class/property_modifier_type_comment/)).
From the **own-line** authoring it reaches the same before-`?` relocation in
every context — including the class property it leaves alone when the comment
is authored inline — via an unstable first pass that crosses the `:` for
signatures (`a?: /* c1 */⏎number;`) and keeps the break for the class property.

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier's first pass lands on the intermediate instead.
- `prettier_intermediate_to_variant_own_line.svelte` — prettier's unstable first
  pass.
- `variant_own_line.svelte` — prettier's landing: all three comments relocated
  before `?`, dual-stable (a comment *authored* before the marker is a match in
  both formatters — [optional_marker_before_comment](../../../types/type_literal/optional_marker_before_comment/)).

The same-gap **line** comment (which forces the break → continuation indent) is
[optional_marker_line_comment](../optional_marker_line_comment_prettier_divergence/);
the plain key→`:` gap without the marker is
[key_colon_own_line_block_comment](../key_colon_own_line_block_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
