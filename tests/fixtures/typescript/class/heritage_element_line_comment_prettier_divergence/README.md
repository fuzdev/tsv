# heritage_element_line_comment_prettier_divergence

Line comments interspersed in a class `implements` (or `extends`) list — a
leading comment on the first element, a trailing comment on an element, and a
leading comment on a later element.

- **tsv**: keeps each comment where the author wrote it — the first element's
  leading comment stays after the keyword, each element on its own line with
  its `,` and comments intact.
- **prettier**: relocates the first element's leading comment up before the
  keyword (`// c⏎implements` instead of `implements⏎// c⏎Z`).

The bug this pins: when the keyword→first-element gap carries a line comment,
the heritage list was joined with a plain `", "`, so a per-element line comment
swallowed the next element (`// c1, B` — non-reparseable content loss). The
list now joins with the comma-baking-aware separators.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md#comment-relocation) §Comment relocation.
