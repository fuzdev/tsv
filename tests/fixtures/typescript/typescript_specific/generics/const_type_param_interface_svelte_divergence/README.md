# const type parameter in interface - Svelte divergence

This fixture tests the `const` type parameter modifier (TS 5.0) on interface type
parameters, in both modifier orderings — the interface analog of
[const_type_param_class](../const_type_param_class/), which acorn now accepts.

## Why tsv Differs

**Acorn-typescript** (used by Svelte's parser) does not support the `const` modifier
on interface type parameters — it rejects the token at parse:

```typescript
interface Single<const T> {} // ❌ Fails in acorn-typescript
interface WithVariance<const in T> {} // ❌ Fails in acorn-typescript (const + variance)
interface VarianceFirst<in const T> {} // ❌ Fails in acorn-typescript (either ordering)
```

TypeScript's own parser **accepts** `const` here and defers the invalidity to a
*checker* error — TS1277 `'const' modifier can only appear on a type parameter of a
function, method or class`. Per the permissive-parser stance (tsc, not acorn, is the
validity oracle), tsv accepts + defers: the AST is produced (see `expected_ours.json`)
and the context-dependent error is left to a future diagnostics layer.

## Two rules, one stance (ordering and context)

Two distinct rules meet on a `const` interface type param, and tsv defers **both** —
because tsc's *parser* accepts both, raising each as a later error:

- **`const` after a variance modifier** (`interface L<in const T>`) breaks the
  declared modifier ORDER. tsc's parser collects modifiers order-free and leaves
  "'const' modifier must precede 'in' modifier" to its grammar checker, so tsv accepts
  and defers. The orderings acorn *also* accepts (both of them, on a class) are pinned
  by [type_param_modifier_order](../type_param_modifier_order/).
- **`const` on an interface** (`interface L<const T>`) is **context-dependent** — valid
  grammar, invalid only because the declaration is an interface (TS1277). tsv accepts +
  defers it too.

acorn draws its line per declaration kind rather than per rule: a **class** type
parameter takes `const`/`in`/`out` in any order, while an interface takes only `in`/`out`
— so every form in this fixture rejects there, and the split above is invisible to it.

The variance-first spelling can only ride as the `unformatted_variance_first` variant:
both formatters normalize the printed modifiers to the canonical `const in`, so
`interface VarianceFirst<in const T>` is nobody's fixed point and cannot be `input`.
The variant is what pins the *acceptance* of that ordering; `input.ts` shows the form
it normalizes to. (Same shape as the class fixture's
`unformatted_reversed_modifier_order` — see
[type_param_modifier_order](../type_param_modifier_order/).)

## Expected behavior

- **tsv parser**: parses all four interfaces (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json`)
- **normalization**: `unformatted_variance_first.ts` writes the last interface as
  `<in const T>`; both formatters normalize it to `input.ts`

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.

## Reference

- [TypeScript 5.0 Release Notes - const Type Parameters](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-0.html#const-type-parameters)
