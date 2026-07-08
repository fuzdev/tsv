# Divergence: line comment between an index-signature key's `:` and its type

A line comment in the key-`:`→key-type gap of an index signature
(`[k: // c\nKeyType]`). The key is a parameter whose type annotation is built by
the shared `build_type_annotation_doc`, so this is the same **uniform
forced-continuation indent** as every other `: Type` context: the comment trails
`:` where the author wrote it and the key type drops to a continuation line
**indented one level**.

- **Union key** (`string | number`, cases A/D) — **diverges**: prettier drops the
  first comment onto its own line (`k:` then the comments and type, all indented);
  tsv keeps the first comment trailing the `:` and indents.
- **Simple key** (`string`, cases B/C/E) — **diverges**: both keep the comment
  trailing `:`, but tsv indents the type one level while prettier leaves it flush.
  Case B also carries a pre-`:` block comment (`k /* x */ :`), which both
  formatters keep inline; only the post-`:` line comment's continuation indent
  differs.

A **block** comment in this gap stays inline in both formatters and is not a
divergence — only a line comment (which runs to EOL, forcing the type onto its
own line) differs. Same rule across every `: Type` annotation context — see
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform forced-continuation indent and §Comment Position Philosophy.
