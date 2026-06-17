# Divergence: colon→type line-comment continuation indents (flush contexts)

A line comment between a `:` annotation and its type
(`X: // c\nType`) — for the contexts where prettier keeps the type **flush**:
variable declarations, class properties, function parameters, function return
types, and **intersection** types in any position.

tsv applies the **uniform forced-continuation indent**: the comment trails `:`
where the author wrote it and the type drops to a continuation line **indented
one level** so it reads as part of the declaration, not a sibling. Prettier
leaves the type flush.

```ts
// tsv (continuation indents one level)   // prettier (flush)
const e: // c1                            const e: // c1
	X = null as any;                      X = null as any;
```

This is one of three faces of the same rule, split by prettier's behavior:

- **prettier keeps flush** (this fixture): variable / class-prop / fn-param /
  return / intersection. tsv indents → divergence.
- **prettier relocates** past the implicit `;` (property signatures): see
  [annotation_simple](../annotation_simple_prettier_divergence/).
- **prettier also indents** (union types): a **match**, not a divergence — see
  the non-divergent [annotation](../annotation/) fixture.

A **block** comment in this gap (`const e: /* c */ X`) stays inline in both
formatters and is not a divergence — only a line comment (which runs to EOL,
forcing the type onto its own line) differs. The single underlying rule lives in
the shared `build_type_annotation_doc`. See
[conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Uniform forced-continuation indent and §Comment Position Philosophy.
