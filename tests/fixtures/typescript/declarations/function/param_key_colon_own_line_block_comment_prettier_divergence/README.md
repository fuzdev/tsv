# param_key_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in a function parameter's name→`:` annotation gap
(`function fn1(⏎a⏎/* c */⏎: string⏎) {}`). A single-line block forces nothing,
and a comment in this gap trails the name (a trailing position), so tsv
collapses the authored breaks and keeps the comment inline in its authored
syntactic slot — the spaced form `function fn1(a /* c */ : string) {}` both
formatters hold stable when authored inline — one pass, the parameter list
re-collapsing with it, runs in order and each comment distinct.

Prettier's chain from the own-line authoring is non-idempotent and splits by
count: pass 1 expands the parameter list and pulls the first comment up to
trail the name, keeping the break before `:`; a **single** parameter comment
then collapses fully on pass 2 to tsv's form, while a **run** parks stable with
the list expanded, the tail comments still own-line and `: type` flush.

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier's first pass lands on the intermediate instead.
- `prettier_intermediate_to_variant_own_line.svelte` — prettier's unstable first
  pass (both lists expanded, breaks kept before `:`).
- `prettier_variant_own_line.svelte` — prettier's landing: the single-comment
  function matches input's line, the run keeps its expanded list; tsv
  normalizes this form to input (the unforced breaks collapse).

The same-gap **line** comment (which forces the break → continuation indent) is
[param_key_colon_line_comment](../param_key_colon_line_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
