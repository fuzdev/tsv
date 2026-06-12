# const type parameter in class - Svelte divergence

This fixture tests the `const` type parameter modifier (TS 5.0) in class declarations.

## Why tsv Differs

**Acorn-typescript** (used by Svelte's parser) does not support the `const` modifier on class type parameters:

```typescript
class Container<const T> {}  // ❌ Fails in acorn-typescript
```

However, it does support `const` on function type parameters:

```typescript
function literal<const T>(value: T): T {}  // ✅ Works in acorn-typescript
```

This is a **limitation in acorn-typescript**, not the tsv parser. TypeScript 5.0+ supports `const` modifiers on all type parameter contexts (classes, interfaces, functions, type aliases).

## Expected behavior

- **tsv parser**: Successfully parses `class Container<const T> {}` (see `expected_ours.json`)
- **Svelte/acorn**: Fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)

## Reference

- [TypeScript 5.0 Release Notes - const Type Parameters](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-0.html#const-type-parameters)
