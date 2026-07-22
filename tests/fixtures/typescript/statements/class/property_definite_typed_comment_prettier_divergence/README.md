# property_definite_typed_comment_prettier_divergence

A class property with a definite-assignment assertion (`!`), a **type
annotation**, and an initializer is a TypeScript early error (TS1263,
"Declarations with initializers cannot also have definite assignment
assertions"). tsv is a deliberately permissive parser — it defers these
static-semantic early errors to a future diagnostics layer (tsc is the
correctness oracle, not the formatter) — so it still parses and formats the
property, preserving a comment authored in the value gap before `=`:

- Input: `e!: number /* c */ = 1;`
- Ours: `e!: number /* c */ = 1;` (idempotent — comment kept where authored)
- Prettier: **rejects** the input (see `prettier_rejects.txt`), so there is no
  prettier oracle for this construct.

The annotation-free sibling (`d! /* c */ = 1;`, no type) is
[property_definite_comment](../property_definite_comment_prettier_divergence/),
which prettier rejects for the complementary reason (TS1264, a definite-assignment
assertion requires a type annotation). Prettier ≤3.9.5 accepted both and
relocated the comment before `!`; Prettier 3.9.6 tightened its TypeScript parser
to reject them at parse time, matching tsc.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
