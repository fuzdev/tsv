# property_modifier_comment_prettier_divergence

A class property with a **no-annotation** `?`/`!` modifier and an initializer.
The `!` form (`b! = 1`) is a TypeScript early error — a definite-assignment
assertion with neither a type annotation (TS1264) nor without an initializer
(TS1263) — while the `?` form (`a? = 1`) is valid. tsv is a deliberately
permissive parser: it defers these static-semantic early errors to a future
diagnostics layer (tsc is the correctness oracle, not the formatter), so it
parses and formats both properties, preserving the author's comment position:

- Input: `a? /* c1 */ = 1;` / `b! /* c2 */ = 1;`
- Ours: unchanged (idempotent — `a? /* c1 */`, `b! /* c2 */` kept where authored)
- Prettier: **rejects** the whole input on the `b!` property (see
  `prettier_rejects.txt`), so there is no prettier oracle for this construct.

Prettier ≤3.9.5 accepted the input and relocated each comment before its
modifier (`a /* c1 */? = 1;` / `b /* c2 */! = 1;`); Prettier 3.9.6 tightened its
TypeScript parser to reject the definite-assignment-without-annotation form
outright, matching tsc, and since both properties share one file it throws on
the whole thing. The former `output_prettier` / `variant_before_modifier` forms
are no longer expressible and have been removed; `unformatted_ours_compact`
still pins tsv's own normalization to the input.

When a type annotation is present (e.g. `a /* c */?: number;` or
`c! /* c */: number;`) both formatters agree and preserve the comment — that
case is the regular `property_modifier_type_comment` fixture (no
`_prettier_divergence`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
