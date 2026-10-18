# type_argument_nested_nullable_union_long_prettier_divergence

**Reason: Print width.** A lone type argument that is one object-like type united
only with `null` / `void` prints as an unbreakable `<>` in both formatters, so an
assignment or type alias over print width breaks after the `=`
(`../type_argument_nullable_union_long/`). When that object-like member has a
type-argument list that can break (`Map<K, V>`; a lone simple argument such as
`B<C>` hugs, so prettier keeps that union unbreakable too), prettier keeps the
outer `<>` unbroken but breaks inside the inner `Map<>` and never reaches the `=`;
tsv keeps the whole type argument on one line and breaks after the `=`, the same
form as a member whose type arguments cannot break: a call's arguments break below
it while its `(` fits after the `=`. When the line after the `=` is still over print
width up to the value's first break (a call's `(`), tsv breaks inside the outer `<>`
instead, keeping the union whole.

- **tsv**: `const [a, b] =⏎\tfn<Map<K, V> | null>(null);`, or with the `(` past
  column 100 after the `=`, `const [a, b] = fn<⏎\tMap<K, V> | null⏎>(null);`
- **Prettier**: `const [a, b] = fn<Map<⏎\tK,⏎\tV⏎> | null>(null);`

Covered: a destructured call (100-char control, the `(` at the 100/101-char
boundary, the line after the `=` at 100/101 chars, the `(` after the `=` at
100/101 chars), a `new` expression and a type alias (100/101-char cases, the line
after the `=` at 100/101 chars).

## Related

- `../type_argument_nullable_union_long/` — the same positions with a member
  whose type arguments cannot break, where both formatters break after the `=`
- `../type_argument_nullable_union_annotation_long_prettier_divergence/` — type
  annotations and return types, where tsv breaks inside the outer `<>`

See [conformance_prettier_ts.md](../../../../../docs/conformance_prettier_ts.md)
§TypeScript (Nullable union type argument) and §Print Width Philosophy.
