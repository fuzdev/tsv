# return_type_generic_union_long_prettier_divergence

**Reason: Print width.** Prettier prints a lone type argument that is one
object-like type united only with `null` / `void` as an unbreakable `<>` (its
`shouldHugUnionType`). When nothing before the return type can break, it leaves
the line over print width; tsv treats printWidth as a hard limit and breaks inside
the `<>` once the line exceeds 100 chars.

tsv: breaks inside the return type generic at the 101-char boundary
Prettier: keeps the `<>` unbroken:

- Function declarations: allows the line to exceed printWidth
- Class methods: allows the line to exceed printWidth

An arrow function assigned to a variable has a break of its own before the
return type: both formatters break at the assignment `=`, so the arrow case is
not a divergence.

Each case has a 100-char control that stays inline in both formatters and a
101-char case.

## Related

- `return_type_generic_union_long/` — non-diverging cases (with `B` instead of
  `null`, so the union is not one prettier keeps unbroken)
- `../../../types/type_argument_nullable_union_long/` — the positions where both
  formatters break before the `<>`
- `../../../types/type_argument_nullable_union_annotation_long_prettier_divergence/` —
  the same divergence in type annotations

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§TypeScript (Nullable union type argument) and §Print Width Philosophy.
