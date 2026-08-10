# import_type_options_comment_svelte_divergence

Comments (and an author blank line) in an import type's two argument gaps —
specifier→options and options→`)`:

```typescript
type T = import('./a' /* c */, { with: { t: 'json' } });
```

The gaps themselves are ordinary: a block before the comma trails the specifier, one
after it leads the options inline, an own-line comment after the options opens the
parens. The divergence is the **options argument**, not the comments — the canonical
parser rejects import-type options outright, so this fixture inherits the
`_svelte_divergence` of [dynamic_attributes](../../modules/imports/dynamic_attributes_svelte_divergence/).

## Status

- **Prettier**: Supports (uses babel-ts parser)
- **Svelte parser (acorn-typescript)**: Does not support (parse error)
- **tsv parser**: Implemented

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md) §TypeScript Corrections.
