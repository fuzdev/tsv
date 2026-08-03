# index_signature_key_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in an index signature's key→`:` gap, inside the
brackets (`[⏎key⏎/* c */⏎: string⏎]: number;`). A single-line block forces
nothing, and a comment in this gap trails the key (a trailing position), so tsv
collapses the authored breaks and keeps the comment inline in its authored
syntactic slot — the spaced form `[key /* c */ : string]: number;` both
formatters hold stable when authored inline — one pass, the bracket
re-collapsing with it, runs in order and each comment distinct.

Prettier's chain from the own-line authoring is non-idempotent and splits by
count: pass 1 keeps the bracket expanded and pulls the first comment up to
trail the key, keeping the break before `:`; a **single** comment then
collapses fully on pass 2 to tsv's form, while a **run** parks stable with the
bracket expanded, the tail comments still own-line and `: string` flush.

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier's first pass lands on the intermediate instead.
- `prettier_intermediate_to_variant_own_line.svelte` — prettier's unstable first
  pass (both brackets expanded, breaks kept before `:`).
- `prettier_variant_own_line.svelte` — prettier's landing: the single-comment
  interface matches input's line, the run keeps its expanded bracket; tsv
  normalizes this form to input (the unforced breaks collapse).

The same-gap **line** comment (which forces the break → continuation indent) is
[index_signature_key_colon_line_comment](../index_signature_key_colon_line_comment_prettier_divergence/);
the `]`→value-`:` gap outside the brackets takes a different prettier outcome
([index_signature_bracket_colon_own_line_block_comment](../index_signature_bracket_colon_own_line_block_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
