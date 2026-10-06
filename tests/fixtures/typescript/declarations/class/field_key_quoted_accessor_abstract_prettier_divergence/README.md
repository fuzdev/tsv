# field_key_quoted_accessor_abstract_prettier_divergence

A quoted class **field** key keeps its quotes ([field_key_quoted](../field_key_quoted/)), and
an auto-accessor and an abstract field are fields. Prettier keeps the quotes on a plain field
and drops them on these.

- **tsv**: `accessor 'a': string`, `static accessor 'c' = 2`, `abstract 'a': string`,
  `abstract accessor 'c': string` — kept quoted
- **Prettier**: `accessor a: string`, `static accessor c = 2`, `abstract a: string`,
  `abstract accessor c: string` — unquoted

Quote style still normalizes (`"a"` → `'a'`, `unformatted_ours_double_quotes`).

## Reason

Prettier's key printer keeps a class field's quotes because TypeScript's
`strictPropertyInitialization` check exempts a string-named field and not an identifier-named
one, and it states that guard as a node type of its own AST — `PropertyDefinition` under its
`typescript` parser. The other field nodes of that AST (`AccessorProperty`,
`TSAbstractPropertyDefinition`, `TSAbstractAccessorProperty`) fall through to the unquoting a
method key takes. In the tree this fixture pins they are all one `PropertyDefinition`.

- **Auto-accessor** (◆prettier_bug): the guard's own reason applies. `accessor 'a': string`
  checks clean under `strict`; prettier's `accessor a: string` is `TS2564: Property 'a' has no
  initializer and is not definitely assigned in the constructor` — formatting creates a type
  error.
- **Abstract field and abstract auto-accessor** (◆design_choice): the check skips an abstract
  member under either spelling, so the quotes are kept for uniformity — one rule for every
  field. Prettier's own TypeScript parsers disagree about the abstract field: `babel-ts`,
  which formats a `<script>` with no `lang`, keeps `abstract 'a'` quoted. Both unquote the
  abstract auto-accessor.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript.
