# line_before_body_comment_prettier_divergence

Line comments between a `while (…)` header's `)` and the body block `{`.

A trailing comment after `)` (`while (a) // c`), an own-line comment before `{`
(`while (a)\n// c\n{`), and a blank line *before* that own-line comment all
normalize to the same form under **both** formatters: tsv drops the blank between
the header `)` and a body-leading comment — matching prettier, and tsv's own
behavior when `{` sits on the header line (`while (a) {\n\n// c` also collapses).
The `unformatted_*` variants pin that shared normalization.

## Divergence

The one difference is a blank line *after* the comment, before `{`
(`while (a)\n// c\n\n{`): **prettier preserves** it, **tsv normalizes** it away
(the body block always hugs the preceding comment). `prettier_variant_spaces.svelte`
pins prettier's stable form; `unformatted_ours_spaces.svelte` is an extra-whitespace
authoring only tsv normalizes back to input.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
