# Divergence: line comment between an index signature's value `:` and its type

A line comment in the value-`:`→type gap of an index signature
(`[k: string]: // c\nType`). The value type annotation is built by the shared
`build_type_annotation_doc`, so this is the index-signature face of the
**uniform forced-continuation indent**: the comment trails `:` where the author
wrote it and the type drops to a continuation line **indented one level** so it
reads as part of the member, not a sibling.

```ts
// tsv (continuation indents one level)   // prettier
[k: string]: // c                         [k: string]: // c
	A & B;                                A & B;
```

- **Union value** (`A | B`, cases I/T) — **matches** prettier: both indent the
  continuation one level.
- **Intersection value** (`A & B`, cases J/U) — **diverges**: tsv indents,
  prettier leaves the type flush.

A **block** comment in this gap (`[k: string]: /* c */ A & B`) stays inline in
both formatters and is not a divergence — only a line comment (which runs to EOL,
forcing the type onto its own line) differs. Same rule across every `: Type`
annotation context — see
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform forced-continuation indent and §Comment Position Philosophy.
