# dynamic_attributes_svelte_divergence

TypeScript import attributes in dynamic `import()` type expressions:

```typescript
type T = import('module', { with: { type: 'json' } }).Foo;
```

## Status

- **Prettier**: Supports (uses babel-ts parser)
- **Svelte parser (acorn-typescript)**: Does not support (parse error)
- **tsv parser**: Implemented

## References

- [Prettier PR #17798](https://github.com/prettier/prettier/pull/17798)

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.
