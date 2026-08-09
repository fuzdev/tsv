# reserved keyword as a type-reference name - Svelte divergence

Every reserved *statement* keyword prettier accepts as a bare type-reference name
(`let x: break;`), plus the three shapes that prove it is a real `TSTypeReference`
rather than a special case: a qualified name (`break.foo`), type arguments
(`yield<T>`), and a union member (`A | default`).

## Why tsv Differs

TypeScript's type space is a **separate namespace**, where a `TypeName` is an
`IdentifierName` — so reserved statement keywords are valid names there. tsc and
prettier both accept `let x: break;`, so tsv parses it as a `TSTypeReference` whose
`typeName` is a plain `Identifier` carrying the keyword text.

**Acorn-typescript** is over-strict in type position and rejects the whole family
(`Unexpected token` / `Expected type`):

```typescript
let break_ref: break; // ❌ acorn
```

acorn-typescript is tsv's AST-**shape** target but not its correctness oracle; for
validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../docs/conformance_svelte.md#typescript-corrections).

`static` / `yield` / `await` / `let` are in the list even though some lex as
contextual identifiers rather than keywords — the end state is the same
`TSTypeReference` however the head token lexes, and pinning them together is what
keeps that true.

## Expected behavior

- **tsv parser**: each annotation is a `TSTypeReference` with an `Identifier` `typeName`;
  `break.foo` is a qualified name, `yield<T>` carries `typeArguments`, and `A | default`
  is a `TSUnionType` whose second member is the reference (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats all of them, and to exactly this input

**The boundary is deliberately NOT here.** Keywords that head their own type
production must *not* collapse to a bare type reference — `void` / `null` / `string`
stay primitive `TSKeyword*` types, `this` a `TSThisType`, `typeof x` a `TSTypeQuery`,
`new () => T` a `TSConstructorType`, `import('m')` a `TSImportType`. acorn **accepts**
all of those, so they are not divergences and cannot share this fixture; they stay as
regression guards in `tests/keyword_type_reference.rs`.
