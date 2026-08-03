# index_signature_bracket_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in an index signature's `]`→value-`:` gap
(`[k: string]⏎/* c */⏎: number;`). A single-line block forces nothing, and a
comment in this gap trails the `]` (a trailing position), so tsv collapses the
authored breaks and keeps the comment inline in its authored syntactic slot —
the spaced form `[k: string] /* c */ : number;` both formatters hold stable
when authored inline (pinned as a match in
[index_signature_bracket_colon_comment](../index_signature_bracket_colon_comment/))
— one pass, runs in order and each comment distinct.

Prettier instead **relocates** the comment into the brackets, trailing the key
type (`[k: string /* c */]: number;`), reaching that relocation
non-idempotently: pass 1 pulls the comment up to trail `]` but keeps the break
before `:`; pass 2 moves it inside the brackets (one comment per pass, so a
run takes 3 passes). The relocated landing is dual-stable — tsv preserves a
comment *authored* inside the brackets — so the two authorings keep two fixed
points, and only prettier moves between them. The run's chain is too long for
a `prettier_intermediate_*` pin (two distinct intermediates), so the README
records it and the fixture documents the divergence by normalization claim
alone — the same shape as
[key_colon_own_line_block_comment](../../../syntax/comments/key_colon_own_line_block_comment_prettier_divergence/).

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier's chain lands on the in-bracket relocation instead (the
  same move its line-comment sibling pins in its `output_prettier.svelte`).

The same-gap **line** comment (which forces the break → continuation indent) is
[index_signature_bracket_colon_line_comment](../index_signature_bracket_colon_line_comment_prettier_divergence/);
the key→`:` gap inside the brackets keeps prettier in the gap instead
([index_signature_key_colon_own_line_block_comment](../index_signature_key_colon_own_line_block_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
