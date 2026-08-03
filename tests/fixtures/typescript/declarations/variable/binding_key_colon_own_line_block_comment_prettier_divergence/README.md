# binding_key_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in a variable binding's name→`:` annotation gap
(`let x⏎/* c */⏎: string;`). A single-line block forces nothing, and a comment
in this gap trails the name (a trailing position), so tsv collapses the
authored breaks and keeps the comment inline in its authored syntactic slot —
the spaced form `let x /* c */ : string;` both formatters hold stable when
authored inline (pinned as a match in
[type_annotation_comment](../type_annotation_comment/)) — one pass, runs in
order and each comment distinct.

Prettier's chain from the own-line authoring is non-idempotent and splits by
count: pass 1 pulls the first comment up to trail the name but keeps the break
before `:` for the rest; a **single** comment then collapses fully on pass 2 to
tsv's form, while a **run** parks stable with the tail comments still own-line
and the `: type` flush on the next line.

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier's first pass lands on the intermediate instead.
- `prettier_intermediate_to_variant_own_line.svelte` — prettier's unstable first
  pass (breaks kept before `:`).
- `prettier_variant_own_line.svelte` — prettier's landing: the single comment
  matches input's line, the run keeps its breaks; tsv normalizes this form to
  input (the run's unforced breaks collapse).

The same-gap **line** comment (which forces the break → continuation indent) is
[binding_key_colon_line_comment](../binding_key_colon_line_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
