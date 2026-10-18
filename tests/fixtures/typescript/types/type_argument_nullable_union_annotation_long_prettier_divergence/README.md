# type_argument_nullable_union_annotation_long_prettier_divergence

**Reason: Print width.** Prettier prints a lone type argument that is one
object-like type united only with `null` / `void` (in any order) as an
unbreakable `<>` (its `shouldHugUnionType`). A type annotation has no break point
before the type, so prettier leaves the line over print width; tsv treats
printWidth as a hard limit and breaks inside the `<>` once the line exceeds 100
chars.

- **tsv**: `a: Promise<⏎\tB | null⏎>;`
- **Prettier**: `a: Promise<B | null>;` past column 100

When the object-like member has a type-argument list that can break
(`Map<K, V>`; a lone simple argument such as `B<C>` hugs, so prettier keeps that
union unbreakable too), as in `Promise<Map<K, V> | null>`, prettier still keeps
the outer `<>` unbroken and breaks inside the inner `Map<>`
(`Promise<Map<⏎\tK,⏎\tV⏎> | null>`). tsv keeps the union whole and breaks inside
the outer `<>`, the same layout as the plain member.

Covered: an interface property (100-char control and 101-char cases, the members
in either order, `void`, a member that has a type-argument list that can break),
a type literal property, a class property, `declare const` and `let` annotations,
and return types with no parameters to break (one with a member that has a
type-argument list that can break).

## Related

- `../type_argument_nullable_union_long/` — the positions where an enclosing
  break keeps the `<>` unbroken in both formatters
- `../type_argument_nested_nullable_union_long_prettier_divergence/` — a member
  that has a type-argument list that can break, in an assignment or type alias,
  where tsv breaks after the `=` instead
- `../../declarations/function/return_type_generic_union_long_prettier_divergence/` —
  the same divergence in return types

See [conformance_prettier_ts.md](../../../../../docs/conformance_prettier_ts.md)
§TypeScript (Nullable union type argument) and §Print Width Philosophy.
