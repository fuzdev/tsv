# Divergence: colon→type line-comment continuation indents (flush contexts)

A line comment between a `:` annotation and its type (`X: // c\nType`), for the
contexts where prettier keeps the type **flush**: variable declarations, class
properties, function parameters, function return types (declaration and arrow),
and **intersection** types in any position (variable and type-element, with one
comment or several).

tsv applies the **uniform forced-continuation indent**: the comment trails `:`
where the author wrote it and the type drops to a continuation line indented one
level, so it reads as part of the declaration, not a sibling. Prettier leaves the
type flush.

This is one of three faces of the same rule, split by prettier's behavior:

- **prettier keeps the type flush** (this fixture): variable / class-prop /
  fn-param / return / intersection. tsv indents → divergence.
- **prettier relocates the comment** past the implicit `;` (property signatures):
  see [annotation_simple](../annotation_simple_prettier_divergence/).
- **prettier drops the comment to its own line** (union types): tsv keeps it
  trailing the `:` → see [annotation](../annotation_prettier_divergence/).

A **block** comment in this gap (`const e: /* c */ X`) stays inline in both
formatters and is not a divergence — only a line comment, which runs to EOL and
forces the type onto its own line, differs. The single underlying rule lives in
the shared `build_type_annotation_doc`.

A redundant paren wrapping the annotation type with the line comment inside
(`const e: (// c\n X)`, and the double-nested `((…))`) strips to this same fixed
point — the `unformatted_ours_single_paren` / `unformatted_ours_double_parens`
variants verify the paren form is idempotent too (across the variable /
class-property / parameter / return contexts). An arrow-function return type
cannot take this form — `(): (// c\n X) =>` parses as a function type, a syntax
error in both formatters — so that context stays bare in the variants.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent and §Comment Position Philosophy.
