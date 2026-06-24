# `await using` Declarations (ES2024 Explicit Resource Management)

## Status

- **Prettier**: Supports `await using` declarations
- **Svelte parser**: Does not support (parse error)
- **tsv parser**: Implemented

## Feature

The `await using` keyword declares an async disposable resource that is automatically disposed and awaited when it goes out of scope, via `Symbol.asyncDispose` or `Symbol.dispose`.

```javascript
async function process() {
  await using resource = getAsyncResource();
  // resource[Symbol.asyncDispose]() called and awaited at scope exit
}
```

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.

## References

- [TC39 Explicit Resource Management](https://github.com/tc39/proposal-explicit-resource-management)
- [MDN: await using declaration](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/await_using)
