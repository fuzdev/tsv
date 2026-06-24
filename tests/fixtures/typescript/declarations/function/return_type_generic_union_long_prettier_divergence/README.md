# return_type_generic_union_long_prettier_divergence

**Reason: Print width.** Prettier special-cases `null`/`void` as the second
member of a union type inside a generic return type, treating printWidth as a
soft target there; tsv treats printWidth as a hard limit and breaks consistently
once the line exceeds 100 chars, regardless of the type keyword.

tsv: breaks inside the return type generic at the 101-char boundary
Prettier: keeps the form it special-cases, varying by declaration kind:

- Function declarations with `null`/`void`: allows the line to exceed printWidth
- Class methods with `null`/`void`: allows the line to exceed printWidth
- Arrow functions with `null`/`void`: breaks at the assignment `=` instead of
  inside the return type

Each case has a 100-char control that stays inline in both formatters and a
101-char case where the divergence appears.

## Related

- `return_type_generic_union_long/` — non-diverging cases (with `B` instead of
  `null`, so prettier's `null`/`void` special-case doesn't fire)

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§TypeScript (Return type generic union) and §Print Width Philosophy.
