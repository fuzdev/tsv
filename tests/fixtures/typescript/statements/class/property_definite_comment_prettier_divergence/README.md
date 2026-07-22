# property_definite_comment_prettier_divergence

A class property with a definite-assignment assertion (`!`) and an initializer
but **no type annotation** is a TypeScript early error (TS1264, "Declarations
with definite assignment assertions must also have type annotations"; the
initializer additionally trips TS1263). tsv is a deliberately permissive parser
— it defers these static-semantic early errors to a future diagnostics layer
(tsc is the correctness oracle, not the formatter) — so it still parses and
formats the property, preserving the author's comment position:

- Input: `d! /* c */ = 1;`
- Ours: `d! /* c */ = 1;` (idempotent — comment kept where authored)
- Prettier: **rejects** the input (see `prettier_rejects.txt`), so there is no
  prettier oracle for this construct.

Prettier ≤3.9.5 accepted the input and relocated the comment before `!`
(`d /* c */! = 1;`); Prettier 3.9.6 tightened its TypeScript parser to reject
the definite-assignment-without-annotation form outright, matching tsc. Because
prettier now throws, the former `output_prettier` / `variant_before_bang` forms
are no longer expressible and have been removed; `unformatted_ours_compact`
still pins tsv's own normalization to the input.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
