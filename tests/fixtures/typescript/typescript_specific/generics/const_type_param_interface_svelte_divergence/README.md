# const type parameter in interface - Svelte divergence

This fixture tests the `const` type parameter modifier (TS 5.0) on interface type
parameters — the interface analog of
[const_type_param_class](../const_type_param_class_svelte_divergence/).

## Why tsv Differs

**Acorn-typescript** (used by Svelte's parser) does not support the `const` modifier
on interface type parameters — it rejects the token at parse:

```typescript
interface Single<const T> {} // ❌ Fails in acorn-typescript
interface WithVariance<const in T> {} // ❌ Fails in acorn-typescript (const + variance)
```

TypeScript's own parser **accepts** `const` here and defers the invalidity to a
*checker* error — TS1277 `'const' modifier can only appear on a type parameter of a
function, method or class`. Per the permissive-parser stance (tsc, not acorn, is the
validity oracle), tsv accepts + defers: the AST is produced (see `expected_ours.json`)
and the context-dependent error is left to a future diagnostics layer.

## The deliberate split (ordering vs context)

Two distinct rules meet on a `const` interface type param — tsv treats them
differently on purpose:

- **`const` after a variance modifier** (`interface L<in const T>`) is a **grammar**
  violation — `const` must *precede* variance. tsv **rejects** it, matching acorn/tsc
  (a syntax error in every context). Pinned by
  [type_param_modifier_order](../type_param_modifier_order/).
- **`const` on an interface** (`interface L<const T>`, or `const` *before* variance
  `<const in T>`) is **context-dependent** — valid grammar, invalid only because the
  declaration is an interface (TS1277). tsv **accepts + defers** it, as here.

So `<in const T>` rejects while `<const T>` / `<const in T>` accept: not an
inconsistency, but the grammar/early-error boundary.

## Expected behavior

- **tsv parser**: parses all three interfaces (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json`)

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.

## Reference

- [TypeScript 5.0 Release Notes - const Type Parameters](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-0.html#const-type-parameters)
