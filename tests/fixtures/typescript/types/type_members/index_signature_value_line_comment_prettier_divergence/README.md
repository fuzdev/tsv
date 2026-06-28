# Divergence: line comment between an index signature's value `:` and its type

A line comment in the value-`:`→type gap of an index signature
(`[k: string]: // c\nType`). The value type annotation is built by the shared
`build_type_annotation_doc`, so this is the index-signature face of the
**uniform forced-continuation indent**: the comment trails `:` where the author
wrote it and the type drops to a continuation line **indented one level** so it
reads as part of the member, not a sibling.

- **Union value** (`A | B`, cases I/T) — **diverges**: prettier drops the comment
  onto its own line (`[k: string]:` then the comment and the union, indented); tsv
  keeps it trailing the `:` and indents the union one level.
- **Intersection value** (`A & B`, cases J/U) — **diverges**: both keep the
  comment trailing `:`, but tsv indents the type one level while prettier leaves
  it flush.
- **Simple value** (`number`, case K) — **diverges**: same as the intersection
  case (comment trails `:`, tsv indents, prettier flush). Index signatures have no
  implicit-`;` end-of-line relocation (unlike a *property* signature, where
  prettier moves a simple type's comment to EOL — see
  [annotation_simple](../../comments/annotation_simple_prettier_divergence/)).

A **block** comment in this gap (`[k: string]: /* c */ A & B`) stays inline in
both formatters and is not a divergence — only a line comment (which runs to EOL,
forcing the type onto its own line) differs. Same rule across every `: Type`
annotation context — see
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent and §Comment Position Philosophy.
