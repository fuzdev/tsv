# heritage_keyword_own_line_block_comment_prettier_divergence

A single-line block comment after a heritage keyword (a class `extends`/`implements`
or an interface `extends`) collapses to the inline form (`class P extends /* a */ Q`)
— whether authored glued, trailing the keyword, or on its own line. The own-line
authoring (`extends⏎/* a */⏎Q`, `unformatted_ours_own_line.svelte`) is what tsv
normalizes here.

- **tsv** collapses to `class P extends /* a */ Q {}` — the comment stays after the
  keyword, on the heritage type, in one pass.
- **Prettier** keeps the comment on its **own line before** the keyword
  (`class P⏎/* a */⏎extends Q {}`) — stable in one pass; it relocates the comment
  across the keyword but, unlike the other relocation gaps, does *not* inline it.

Heritage is the odd one out among the relocation gaps: the other gaps
(`as`/`satisfies`, predicate `is`, conditional `extends`, type-param
`extends`/`=`, indexed access) reach an *inline* relocated form
(`x /* c */ as A`) over two passes, but Prettier keeps the heritage comment
broken onto its own line — in one pass, stably. Prettier's stable form
(`class P⏎/* a */⏎extends Q`) is **not** dual-stable: tsv re-collapses it to a
*third* form (`class P /* a */ extends Q` — comment before the keyword, inline).
A `variant_*` file is meant to be dual-stable (both formatters keep it verbatim),
so this three-way case has no dedicated pin type — it is documented by this README
plus the `unformatted_ours_own_line` variant, and the fixture carries the
validator's informational "undocumented Prettier output" note by design. (It is
one of a small recurring family of "Prettier keeps V, tsv rewrites V to a third
stable form" cases — see the conformance-doc note for the others.) Per
[Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the heritage type. Only a
**line** comment
([extends_keyword_line_comment](../extends_keyword_line_comment_prettier_divergence/))
or an **own-line multiline** block comment still hangs the heritage type on its own line (a glued multiline block collapses inline).
Covers class `extends`, class `implements`, and interface `extends`.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
