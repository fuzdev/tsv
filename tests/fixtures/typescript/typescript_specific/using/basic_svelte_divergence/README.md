# `using` Declarations (Explicit Resource Management)

## Status

- **Prettier**: Supports `using` declarations
- **Svelte parser**: Does not support (parse error)
- **tsv parser**: Implemented

## Feature

The `using` keyword declares a block-scoped variable that is automatically disposed when it goes out of scope, via `Symbol.dispose`.

```javascript
using resource = getResource();
// resource[Symbol.dispose]() called automatically at scope exit
```

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.

## References

- [TC39 Explicit Resource Management](https://github.com/tc39/proposal-explicit-resource-management)
- [MDN: using declaration](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/using)
